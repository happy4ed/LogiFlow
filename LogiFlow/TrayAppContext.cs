using System;
using System.Collections.Generic;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Handoff;
using LogiFlow.Ui;

namespace LogiFlow
{
    /// <summary>
    /// 트레이 상주 앱의 중심. 설정·스캔·엣지 감시·전환·기기 캐시를 소유한다.
    /// 시작 시 캐시로 바로 표시하고, 「새로 고침」·기기 변화 때만 재스캔한다.
    /// </summary>
    internal sealed class TrayAppContext : ApplicationContext
    {
        public DiagLogger Log { get; private set; }
        public AppConfig Config { get; private set; }
        public List<ScanResult> LastResults { get; private set; }

        private NotifyIcon _tray;
        private ContextMenuStrip _menu;
        private EdgeWatcher _watcher;
        private GlowController _glow;
        private MainForm _settings;
        private bool _scanning;
        private bool _exiting;
        private string _deviceSignature = "";
        private long _lastDeviceChangeTick;

        public TrayAppContext(DiagLogger log)
        {
            Log = log;
            Config = AppConfig.Load();

            // 설정 적용(테마·글자크기·로그).
            Theme.SetMode(Config.Theme);
            UiFont.SetOffset(Config.FontOffset);
            Log.Enabled = Config.DiagLogEnabled;

            // 캐시로 즉시 표시(매번 재검색 안 함).
            LastResults = DeviceCache.Load();

            _glow = new GlowController();
            _watcher = new EdgeWatcher(log, OnEdgeTrigger, _glow.Update);

            _settings = new MainForm(this);
            var forceHandle = _settings.Handle;
            GC.KeepAlive(forceHandle);

            BuildTray();
            AutoStart.Apply(Config.AutoStart);

            ReconfigureWatcher();
            UpdateTray(null);

            // 캐시가 비었으면 첫 스캔, 아니면 캐시 표시 유지.
            _settings.BeginInvoke((Action)(() =>
            {
                _deviceSignature = DeviceScanner.Signature();
                if (LastResults.Count == 0)
                {
                    Rescan();
                }
                else
                {
                    _settings.OnStateUpdated();
                }
            }));
        }

        private void BuildTray()
        {
            _menu = new ContextMenuStrip();
            _menu.Opening += (s, e) => RebuildMenu();

            _tray = new NotifyIcon
            {
                Icon = AppIcon.Get(),
                Visible = true,
                Text = "LogiFlow",
                ContextMenuStrip = _menu,
            };
            _tray.DoubleClick += (s, e) => ShowSettings();
            RebuildMenu();
        }

        private void RebuildMenu()
        {
            _menu.Items.Clear();
            int cur = CurrentChannel();
            int maxHost = MaxHostCount();

            _menu.Items.Add(new ToolStripMenuItem(
                cur >= 0 ? string.Format("현재 채널: {0} · {1}", cur + 1, Config.AliasFor(cur)) : "현재 채널: (미탐지)") { Enabled = false });
            _menu.Items.Add(new ToolStripSeparator());

            bool any = false;
            for (int h = 0; h < maxHost; h++)
            {
                if (h == cur) continue;
                any = true;
                int target = h;
                var item = new ToolStripMenuItem(string.Format("채널 {0} · {1}", h + 1, Config.AliasFor(h)));
                item.Click += (s, e) => QuickSwitch(target);
                _menu.Items.Add(item);
            }
            if (!any)
            {
                _menu.Items.Add(new ToolStripMenuItem("전환할 채널 없음") { Enabled = false });
            }

            _menu.Items.Add(new ToolStripSeparator());
            _menu.Items.Add(new ToolStripMenuItem("설정 열기", null, (s, e) => ShowSettings()));
            _menu.Items.Add(new ToolStripMenuItem("새로 고침(스캔)", null, (s, e) => Rescan()));
            _menu.Items.Add(new ToolStripSeparator());
            _menu.Items.Add(new ToolStripMenuItem("종료", null, (s, e) => ExitApp()));
        }

        public List<ScanResult> Switchable()
        {
            var list = new List<ScanResult>();
            foreach (var r in LastResults)
            {
                if (r.ChangeHostSupported && r.HostCount > 0) list.Add(r);
            }
            return list;
        }

        public int CurrentChannel()
        {
            foreach (var r in Switchable())
            {
                if (r.CurrentHost >= 0) return r.CurrentHost;
            }
            return -1;
        }

        public int MaxHostCount()
        {
            int max = 0;
            foreach (var r in Switchable())
            {
                if (r.HostCount > max) max = r.HostCount;
            }
            return max;
        }

        private void QuickSwitch(int channel)
        {
            Log.Log(string.Format("트레이 빠른 전환 → 채널 {0}", channel + 1));
            List<ScanResult> devices = Switchable();
            var t = new Thread(() =>
            {
                int sent = HostSwitcher.SwitchAll(devices, channel, Log);
                Log.Log(string.Format("트레이 전환 전송: {0}개", sent));
                try { _settings.BeginInvoke((Action)(() => UpdateTray(null))); } catch { }
            });
            t.IsBackground = true; t.Start();
        }

        public void Rescan()
        {
            if (_scanning) return;
            _scanning = true;
            UpdateTray("스캔 중...");
            var t = new Thread(() =>
            {
                List<ScanResult> results;
                try { results = new DeviceScanner(Log).Scan(); }
                catch (Exception ex) { results = new List<ScanResult>(); Log.LogError("스캔 예외: " + ex); }
                string sig = DeviceScanner.Signature();
                try
                {
                    _settings.BeginInvoke((Action)(() =>
                    {
                        LastResults = results;
                        _deviceSignature = sig;
                        _scanning = false;
                        DeviceCache.Save(results);
                        // 이 PC 의 호스트(채널)를 기억 — 로그온 전 '당기기' 대상. 기기가 지금 여기 있을 때만 유효.
                        int myHost = CurrentChannel();
                        if (myHost >= 0 && Config.MyHost != myHost)
                        {
                            Config.MyHost = myHost;
                            Config.Save();
                        }
                        ReconfigureWatcher();
                        UpdateTray(null);
                        _settings.OnStateUpdated();
                    }));
                }
                catch { _scanning = false; }
            });
            t.IsBackground = true; t.Start();
        }

        /// <summary>WM_DEVICECHANGE 수신 시(MainForm 경유). 지문이 바뀌었을 때만 재스캔.</summary>
        public void OnDeviceChanged()
        {
            long now = Environment.TickCount;
            if (now - _lastDeviceChangeTick < 1200) return; // 디바운스
            _lastDeviceChangeTick = now;
            var t = new Thread(() =>
            {
                Thread.Sleep(600); // 장치 안정화 대기
                string sig = DeviceScanner.Signature();
                if (sig != _deviceSignature)
                {
                    Log.Log("기기 변화 감지 → 재스캔");
                    try { _settings.BeginInvoke((Action)Rescan); } catch { }
                }
            });
            t.IsBackground = true; t.Start();
        }

        public void ApplyConfig()
        {
            Config.Save();
            AutoStart.Apply(Config.AutoStart);
            Log.Enabled = Config.DiagLogEnabled;
            ReconfigureWatcher();
            UpdateTray(null);
        }

        private void ReconfigureWatcher()
        {
            _watcher.Stop();
            _watcher.Configure(Config);
            if (Config.EdgeSwitchEnabled && _watcher.HasAttachments && Switchable().Count > 0)
            {
                _watcher.Start();
            }
        }

        private void OnEdgeTrigger(int channel)
        {
            Log.Log(string.Format("엣지 트리거 수신 → 채널 {0} 전환", channel + 1));
            List<ScanResult> devices = Switchable();
            var t = new Thread(() =>
            {
                int sent = HostSwitcher.SwitchAll(devices, channel, Log);
                Log.Log(string.Format("엣지 전환 전송: {0}개", sent));
                try { _settings.BeginInvoke((Action)(() => UpdateTray(null))); } catch { }
            });
            t.IsBackground = true; t.Start();
        }

        private void UpdateTray(string statusOverride)
        {
            int cur = CurrentChannel();
            string title = "LogiFlow";
            if (statusOverride != null) title += " — " + statusOverride;
            else if (cur >= 0) title += string.Format(" — 채널 {0} · {1}", cur + 1, Config.AliasFor(cur));
            // NotifyIcon.Text 최대 63자.
            if (title.Length > 62) title = title.Substring(0, 62);
            if (_tray != null) _tray.Text = title;
        }

        public void ShowSettings()
        {
            _settings.ShowSettings();
        }

        private void ExitApp()
        {
            if (_exiting) return;
            _exiting = true;
            try { _watcher.Dispose(); } catch { }
            try { _glow.Dispose(); } catch { }
            try { _tray.Visible = false; _tray.Dispose(); } catch { }
            Log.Log("LogiFlow 종료(트레이)");
            ExitThread();
        }
    }
}
