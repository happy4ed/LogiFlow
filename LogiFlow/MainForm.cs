using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;

namespace LogiFlow
{
    /// <summary>
    /// 설정 창(트레이에서 열림). 탭1: 진단·수동 전환, 탭2: 엣지 자동 전환 배치.
    /// 창을 닫으면 트레이로 내려간다(앱은 계속 상주).
    /// </summary>
    internal sealed class MainForm : Form
    {
        private readonly TrayAppContext _tray;
        private DiagLogger Log { get { return _tray.Log; } }

        // 탭1
        private CheckBox _showAllCheck;
        private TextBox _output;
        private Panel _transferPanel;

        // 탭2
        private MonitorLayoutPanel _canvas;
        private CheckBox _edgeEnableCheck;
        private NumericUpDown _dwellUpDown;
        private CheckBox _autoStartCheck;

        private bool _busy;

        public MainForm(TrayAppContext tray)
        {
            _tray = tray;
            BuildUi();
        }

        private void BuildUi()
        {
            Text = "LogiFlow — 설정";
            Width = 820;
            Height = 680;
            MinimumSize = new Size(640, 520);
            StartPosition = FormStartPosition.CenterScreen;
            Font = new Font("Segoe UI", 9f);
            ShowInTaskbar = true;

            var tabs = new TabControl { Dock = DockStyle.Fill };
            tabs.TabPages.Add(BuildDiagnosticsTab());
            tabs.TabPages.Add(BuildEdgeTab());
            Controls.Add(tabs);

            FormClosing += OnFormClosing;
        }

        private TabPage BuildDiagnosticsTab()
        {
            var page = new TabPage("진단 · 수동 전환");

            var top = new Panel { Dock = DockStyle.Top, Height = 40, Padding = new Padding(8) };
            var refreshBtn = new Button { Text = "새로 고침", Width = 90, Height = 26, Left = 8, Top = 6 };
            refreshBtn.Click += (s, e) => _tray.Rescan();
            var logBtn = new Button { Text = "로그 폴더 열기", Width = 110, Height = 26, Left = 102, Top = 6 };
            logBtn.Click += (s, e) => OpenLogFolder();
            _showAllCheck = new CheckBox { Text = "모두 보기(진단)", Left = 222, Top = 10, AutoSize = true, Checked = false };
            _showAllCheck.CheckedChanged += (s, e) => RenderResults();
            top.Controls.Add(refreshBtn);
            top.Controls.Add(logBtn);
            top.Controls.Add(_showAllCheck);

            _transferPanel = new Panel { Dock = DockStyle.Bottom, Height = 190, Padding = new Padding(8), BorderStyle = BorderStyle.FixedSingle, AutoScroll = true };

            _output = new TextBox
            {
                Dock = DockStyle.Fill,
                Multiline = true,
                ReadOnly = true,
                ScrollBars = ScrollBars.Both,
                WordWrap = false,
                Font = new Font("Consolas", 9.5f),
                BackColor = Color.White,
            };

            page.Controls.Add(_output);
            page.Controls.Add(top);
            page.Controls.Add(_transferPanel);
            return page;
        }

        private TabPage BuildEdgeTab()
        {
            var page = new TabPage("엣지 자동 전환");

            var bottom = new Panel { Dock = DockStyle.Bottom, Height = 132, Padding = new Padding(10) };

            _edgeEnableCheck = new CheckBox { Text = "엣지 자동 전환 사용", Left = 10, Top = 8, AutoSize = true, Checked = _tray.Config.EdgeSwitchEnabled };

            var dwellLabel = new Label { Text = "엣지 유지(ms):", Left = 10, Top = 36, AutoSize = true };
            _dwellUpDown = new NumericUpDown { Left = 110, Top = 32, Width = 70, Minimum = 100, Maximum = 2000, Increment = 10, Value = Clamp(_tray.Config.DwellMs, 100, 2000) };

            _autoStartCheck = new CheckBox { Text = "로그인 시 자동 시작", Left = 10, Top = 62, AutoSize = true, Checked = _tray.Config.AutoStart };

            var warn = new Label
            {
                Left = 10, Top = 88, Width = 560, Height = 34,
                ForeColor = Color.FromArgb(176, 0, 32),
                Text = "엣지에 닿으면 마우스·키보드가 그 채널 PC 로 넘어갑니다. 돌아오려면 그 PC 에서 "
                     + "채널 1 을 누르거나 기기의 Easy-Switch 버튼을 쓰세요(이 PC 는 기기가 떠나 명령을 보낼 수 없음).",
            };

            var saveBtn = new Button { Text = "저장", Left = 600, Top = 60, Width = 90, Height = 30 };
            saveBtn.Click += (s, e) => SaveEdgeConfig();

            bottom.Controls.Add(_edgeEnableCheck);
            bottom.Controls.Add(dwellLabel);
            bottom.Controls.Add(_dwellUpDown);
            bottom.Controls.Add(_autoStartCheck);
            bottom.Controls.Add(warn);
            bottom.Controls.Add(saveBtn);

            _canvas = new MonitorLayoutPanel { Dock = DockStyle.Fill };

            page.Controls.Add(_canvas);
            page.Controls.Add(bottom);
            return page;
        }

        private static int Clamp(int v, int lo, int hi)
        {
            if (v < lo) return lo;
            if (v > hi) return hi;
            return v;
        }

        public void ShowSettings()
        {
            if (!Visible)
            {
                Show();
            }
            if (WindowState == FormWindowState.Minimized)
            {
                WindowState = FormWindowState.Normal;
            }
            Activate();
            BringToFront();
            OnStateUpdated();
        }

        private void OnFormClosing(object sender, FormClosingEventArgs e)
        {
            // 사용자가 X 로 닫으면 종료하지 않고 트레이로 내려간다.
            if (e.CloseReason == CloseReason.UserClosing)
            {
                e.Cancel = true;
                Hide();
            }
        }

        /// <summary>트레이에서 스캔/상태 갱신 시 호출. 목록·캔버스 새로 그림.</summary>
        public void OnStateUpdated()
        {
            RenderResults();
            RefreshCanvas();
        }

        private void RefreshCanvas()
        {
            if (_canvas == null) return;
            int maxHost = _tray.MaxHostCount();
            int current = _tray.CurrentChannel();
            var channels = new List<int>();
            for (int h = 0; h < maxHost; h++)
            {
                if (h != current)
                {
                    channels.Add(h);
                }
            }
            _canvas.SetData(channels, current, _tray.Config.Attachments);
        }

        private void SaveEdgeConfig()
        {
            _tray.Config.EdgeSwitchEnabled = _edgeEnableCheck.Checked;
            _tray.Config.DwellMs = (int)_dwellUpDown.Value;
            _tray.Config.AutoStart = _autoStartCheck.Checked;
            _tray.Config.Attachments.Clear();
            _tray.Config.Attachments.AddRange(_canvas.GetAttachments());
            _tray.ApplyConfig();
            MessageBox.Show(this,
                string.Format("저장했습니다. 엣지 배치 {0}개.\r\n엣지 자동 전환: {1}",
                    _tray.Config.Attachments.Count, _edgeEnableCheck.Checked ? "켬" : "끔"),
                "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
        }

        // ---- 진단·전환 탭 ----

        private List<ScanResult> Switchable()
        {
            return _tray.Switchable();
        }

        private void RenderResults()
        {
            if (_output == null) return;
            bool showAll = _showAllCheck.Checked;
            var shown = new List<ScanResult>();
            foreach (var r in _tray.LastResults)
            {
                if (showAll || r.ChangeHostSupported)
                {
                    shown.Add(r);
                }
            }
            _output.Text = FormatReport(shown, _tray.LastResults.Count, showAll);
            BuildTransferPanel();
        }

        private static string FormatReport(List<ScanResult> shown, int totalCount, bool showAll)
        {
            var sb = new StringBuilder();
            sb.AppendLine("LogiFlow HID++ 진단 결과");
            sb.AppendLine(new string('=', 60));
            sb.AppendLine(showAll
                ? string.Format("전체 응답 장치: {0}개 (진단 모드)", totalCount)
                : string.Format("전환 가능 장치: {0}개  (전체 {1}개 중 · 빈 슬롯/리시버 숨김)", shown.Count, totalCount));
            sb.AppendLine();

            if (shown.Count == 0)
            {
                sb.AppendLine("표시할 장치가 없습니다. 기기 연결 확인 후 [새로 고침].");
                return sb.ToString();
            }

            int n = 0;
            foreach (var r in shown)
            {
                n++;
                sb.AppendLine(string.Format("[{0}] {1}", n, r.DeviceName));
                sb.AppendLine(string.Format("    VID:PID       {0}", r.VidPid));
                sb.AppendLine(string.Format("    연결 방식      {0}", r.ConnectionType));
                sb.AppendLine(string.Format("    deviceIndex   0x{0:X2}", r.DeviceIndex));
                if (r.ChangeHostSupported)
                {
                    sb.AppendLine(string.Format("    0x1814 인덱스  0x{0:X2}", r.ChangeHostIndex));
                    if (r.HostCount >= 0)
                    {
                        sb.AppendLine(string.Format("    호스트 개수    {0}", r.HostCount));
                        sb.AppendLine(string.Format("    현재 채널      {0}  (채널 {1})", r.CurrentHost, r.CurrentHost + 1));
                    }
                }
                else
                {
                    sb.AppendLine("    0x1814 인덱스  미지원");
                }
                if (!string.IsNullOrEmpty(r.Note))
                {
                    sb.AppendLine(string.Format("    비고          {0}", r.Note));
                }
                sb.AppendLine();
            }
            return sb.ToString();
        }

        private void BuildTransferPanel()
        {
            _transferPanel.Controls.Clear();
            var switchable = Switchable();
            if (switchable.Count == 0)
            {
                _transferPanel.Controls.Add(new Label { AutoSize = true, Left = 8, Top = 8, Text = "전환 가능한 기기가 없습니다. (0x1814 지원 기기 미탐지)" });
                return;
            }

            int maxHost = _tray.MaxHostCount();
            int y = 6;

            _transferPanel.Controls.Add(new Label
            {
                AutoSize = true, Left = 6, Top = y, Font = new Font(Font, FontStyle.Bold),
                Text = string.Format("수동 전환 — 전환 가능 기기 {0}개", switchable.Count),
            });
            y += 22;

            _transferPanel.Controls.Add(new Label
            {
                AutoSize = false, Left = 6, Top = y, Width = 740, Height = 30,
                ForeColor = Color.FromArgb(176, 0, 32),
                Text = "누르면 마우스·키보드가 해당 PC 로 넘어갑니다. 돌아오려면 그 PC 에서 채널 1 또는 Easy-Switch 버튼.",
            });
            y += 34;

            _transferPanel.Controls.Add(new Label { AutoSize = true, Left = 6, Top = y + 4, Text = "모두 함께:" });
            int bx = 78;
            for (int h = 0; h < maxHost; h++)
            {
                int target = h;
                var b = new Button { Text = string.Format("채널 {0}로", h + 1), Width = 76, Height = 26, Left = bx, Top = y };
                b.Click += (s, e) => DoSwitchAll(target);
                _transferPanel.Controls.Add(b);
                bx += 82;
            }
            y += 34;

            foreach (var d in switchable)
            {
                string cur = d.CurrentHost >= 0 ? (d.CurrentHost + 1).ToString() : "?";
                _transferPanel.Controls.Add(new Label
                {
                    AutoSize = true, Left = 6, Top = y + 4,
                    Text = string.Format("{0}  (현재 채널 {1})", d.DeviceName, cur),
                });
                int dbx = 360;
                int hosts = d.HostCount > 0 ? d.HostCount : maxHost;
                for (int h = 0; h < hosts; h++)
                {
                    int target = h;
                    ScanResult dev = d;
                    var b = new Button { Text = (h + 1).ToString(), Width = 34, Height = 24, Left = dbx, Top = y };
                    b.Click += (s, e) => DoSwitchOne(dev, target);
                    _transferPanel.Controls.Add(b);
                    dbx += 38;
                }
                y += 30;
            }

            foreach (Control c in _transferPanel.Controls)
            {
                if (c is Button) c.Enabled = !_busy;
            }
        }

        private bool ConfirmSwitch(string what, int targetHost)
        {
            return MessageBox.Show(this,
                string.Format("{0} 를 채널 {1} 로 전환합니다.\r\n\r\n해당 PC 로 넘어가며 이 PC 에서는 기기가 사라집니다.\r\n진행할까요?",
                    what, targetHost + 1),
                "LogiFlow 전환 확인", MessageBoxButtons.YesNo, MessageBoxIcon.Warning) == DialogResult.Yes;
        }

        private void DoSwitchAll(int targetHost)
        {
            if (_busy) return;
            if (!ConfirmSwitch("전환 가능한 모든 기기", targetHost)) return;
            RunSwitch(() =>
            {
                int sent = HostSwitcher.SwitchAll(Switchable(), targetHost, Log);
                return string.Format("채널 {0}로 전체 전환: {1}개 전송", targetHost + 1, sent);
            });
        }

        private void DoSwitchOne(ScanResult device, int targetHost)
        {
            if (_busy) return;
            if (!ConfirmSwitch(device.DeviceName, targetHost)) return;
            RunSwitch(() =>
            {
                bool ok = HostSwitcher.SwitchOne(device, targetHost, Log);
                return string.Format("{0} → 채널 {1}: {2}", device.DeviceName, targetHost + 1, ok ? "전송" : "실패(로그 확인)");
            });
        }

        private void RunSwitch(Func<string> action)
        {
            _busy = true;
            BuildTransferPanel();
            var thread = new Thread(() =>
            {
                string msg;
                try { msg = action(); }
                catch (Exception ex) { msg = "전환 오류: " + ex.Message; Log.LogError("전환 오류: " + ex); }
                try
                {
                    BeginInvoke(new Action(() =>
                    {
                        _busy = false;
                        BuildTransferPanel();
                        MessageBox.Show(this, msg + "\r\n\r\n전환된 기기는 이 PC 에서 사라집니다. [새로 고침] 으로 다시 읽을 수 있습니다.",
                            "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
                    }));
                }
                catch { _busy = false; }
            });
            thread.IsBackground = true;
            thread.Start();
        }

        private void OpenLogFolder()
        {
            try
            {
                Directory.CreateDirectory(Log.LogDirectory);
                Process.Start("explorer.exe", "\"" + Log.LogDirectory + "\"");
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "로그 폴더를 열지 못했습니다:\r\n" + ex.Message,
                    "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }
    }
}
