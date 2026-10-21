using System;
using System.Collections.Generic;
using System.Drawing;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Handoff;

namespace LogiFlow
{
    /// <summary>
    /// 트레이 상주 앱의 중심. 설정·스캔·엣지 감시·전환을 소유한다.
    /// 설정 창(MainForm)은 트레이에서 연다. 창을 닫으면 트레이로 내려간다.
    /// </summary>
    internal sealed class TrayAppContext : ApplicationContext
    {
        public DiagLogger Log { get; private set; }
        public AppConfig Config { get; private set; }
        public List<ScanResult> LastResults { get; private set; }

        private NotifyIcon _tray;
        private ToolStripMenuItem _channelItem;
        private ToolStripMenuItem _edgeStateItem;
        private EdgeWatcher _watcher;
        private MainForm _settings;
        private bool _scanning;
        private bool _exiting;

        public TrayAppContext(DiagLogger log)
        {
            Log = log;
            LastResults = new List<ScanResult>();
            Config = AppConfig.Load();

            _watcher = new EdgeWatcher(log, OnEdgeTrigger);

            _settings = new MainForm(this);
            // 핸들을 미리 만들어(숨긴 채) BeginInvoke 마셜링에 쓴다.
            var forceHandle = _settings.Handle;
            GC.KeepAlive(forceHandle);

            BuildTray();

            // 레지스트리 자동시작을 설정과 일치시킨다.
            AutoStart.Apply(Config.AutoStart);

            // 메시지 루프가 시작된 뒤 첫 스캔.
            _settings.BeginInvoke((Action)Rescan);
        }

        private void BuildTray()
        {
            var menu = new ContextMenuStrip();
            _channelItem = new ToolStripMenuItem("현재 채널: 조회 중...") { Enabled = false };
            _edgeStateItem = new ToolStripMenuItem("엣지 자동전환: -") { Enabled = false };
            menu.Items.Add(_channelItem);
            menu.Items.Add(_edgeStateItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(new ToolStripMenuItem("설정 열기", null, (s, e) => ShowSettings()));
            menu.Items.Add(new ToolStripMenuItem("새로 고침(스캔)", null, (s, e) => Rescan()));
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add(new ToolStripMenuItem("종료", null, (s, e) => ExitApp()));

            _tray = new NotifyIcon
            {
                Icon = SystemIcons.Application,
                Visible = true,
                Text = "LogiFlow",
                ContextMenuStrip = menu,
            };
            _tray.DoubleClick += (s, e) => ShowSettings();
        }

        public List<ScanResult> Switchable()
        {
            var list = new List<ScanResult>();
            foreach (var r in LastResults)
            {
                if (r.ChangeHostSupported && r.HostCount > 0)
                {
                    list.Add(r);
                }
            }
            return list;
        }

        /// <summary>전환 가능 기기의 대표 현재 채널(0-base). 없으면 -1.</summary>
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

        public void Rescan()
        {
            if (_scanning) return;
            _scanning = true;
            UpdateTray("스캔 중...");
            var t = new Thread(() =>
            {
                List<ScanResult> results;
                try
                {
                    results = new DeviceScanner(Log).Scan();
                }
                catch (Exception ex)
                {
                    results = new List<ScanResult>();
                    Log.LogError("스캔 예외: " + ex);
                }
                try
                {
                    _settings.BeginInvoke((Action)(() =>
                    {
                        LastResults = results;
                        _scanning = false;
                        ReconfigureWatcher();
                        UpdateTray(null);
                        _settings.OnStateUpdated();
                    }));
                }
                catch
                {
                    _scanning = false;
                }
            });
            t.IsBackground = true;
            t.Start();
        }

        /// <summary>설정 적용: 저장 + 자동시작 + 엣지 감시 재구성.</summary>
        public void ApplyConfig()
        {
            Config.Save();
            AutoStart.Apply(Config.AutoStart);
            ReconfigureWatcher();
            UpdateTray(null);
        }

        private void ReconfigureWatcher()
        {
            _watcher.Stop();
            _watcher.Configure(Config);
            bool canRun = Config.EdgeSwitchEnabled && _watcher.HasAttachments && Switchable().Count > 0;
            if (canRun)
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
                try
                {
                    _settings.BeginInvoke((Action)(() => UpdateTray(null)));
                }
                catch
                {
                }
            });
            t.IsBackground = true;
            t.Start();
        }

        private void UpdateTray(string statusOverride)
        {
            int cur = CurrentChannel();
            string chanText = cur >= 0 ? string.Format("현재 채널: {0}", cur + 1) : "현재 채널: (미탐지)";
            _channelItem.Text = statusOverride ?? chanText;

            string edge;
            if (!Config.EdgeSwitchEnabled) edge = "엣지 자동전환: 꺼짐";
            else if (!_watcher.HasAttachments) edge = "엣지 자동전환: 배치 없음";
            else if (_watcher.IsRunning) edge = "엣지 자동전환: 켜짐";
            else edge = "엣지 자동전환: 대기";
            _edgeStateItem.Text = edge;

            _tray.Text = "LogiFlow — " + (cur >= 0 ? "채널 " + (cur + 1) : "진단");
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
            try { _tray.Visible = false; _tray.Dispose(); } catch { }
            Log.Log("LogiFlow 종료(트레이)");
            ExitThread();
        }
    }
}
