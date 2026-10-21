using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;
using LogiFlow.Diagnostics;

namespace LogiFlow
{
    /// <summary>
    /// 진단 + 전환 테스트 창. 전환 가능한 기기(마우스·키보드)만 기본 표시하고,
    /// 채널 전환 버튼을 제공한다. 전환(setHost)은 버튼 클릭 때만 호출된다.
    /// </summary>
    internal sealed class MainForm : Form
    {
        private readonly DiagLogger _log;
        private Button _refreshButton;
        private Button _openLogButton;
        private CheckBox _showAllCheck;
        private Label _statusLabel;
        private TextBox _output;
        private Panel _transferPanel;

        private List<ScanResult> _allResults = new List<ScanResult>();
        private bool _busy;

        public MainForm(DiagLogger log)
        {
            _log = log;
            BuildUi();
        }

        private void BuildUi()
        {
            Text = "LogiFlow — HID++ 진단 · 전환 테스트";
            Width = 780;
            Height = 640;
            MinimumSize = new Size(600, 480);
            StartPosition = FormStartPosition.CenterScreen;
            Font = new Font("Segoe UI", 9f);

            var topPanel = new Panel { Dock = DockStyle.Top, Height = 44, Padding = new Padding(8) };

            _refreshButton = new Button { Text = "새로 고침", Width = 90, Height = 28, Left = 8, Top = 8 };
            _refreshButton.Click += (s, e) => StartScan();

            _openLogButton = new Button { Text = "로그 폴더 열기", Width = 110, Height = 28, Left = 102, Top = 8 };
            _openLogButton.Click += (s, e) => OpenLogFolder();

            _showAllCheck = new CheckBox { Text = "모두 보기(진단)", Left = 222, Top = 12, AutoSize = true, Checked = false };
            _showAllCheck.CheckedChanged += (s, e) => RenderResults();

            _statusLabel = new Label { AutoSize = true, Left = 350, Top = 14, Text = "준비됨" };

            topPanel.Controls.Add(_refreshButton);
            topPanel.Controls.Add(_openLogButton);
            topPanel.Controls.Add(_showAllCheck);
            topPanel.Controls.Add(_statusLabel);

            _transferPanel = new Panel
            {
                Dock = DockStyle.Bottom,
                Height = 190,
                Padding = new Padding(8),
                BorderStyle = BorderStyle.FixedSingle,
                AutoScroll = true,
            };

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

            Controls.Add(_output);
            Controls.Add(topPanel);
            Controls.Add(_transferPanel);

            Shown += (s, e) => StartScan();
        }

        private void StartScan()
        {
            if (_busy)
            {
                return;
            }
            SetBusy(true, "스캔 중...");
            _output.Text = "로지텍 HID++ 장치를 읽는 중입니다...";

            var thread = new Thread(() =>
            {
                List<ScanResult> results;
                string error = null;
                try
                {
                    var scanner = new DeviceScanner(_log);
                    results = scanner.Scan();
                }
                catch (Exception ex)
                {
                    results = new List<ScanResult>();
                    error = ex.ToString();
                    _log.LogError("스캔 예외: " + ex);
                }

                BeginInvoke(new Action(() =>
                {
                    _allResults = results;
                    if (error != null)
                    {
                        _output.Text = "스캔 중 오류가 발생했습니다.\r\n\r\n" + error;
                        _statusLabel.Text = "오류 — 로그 확인";
                        ClearTransferPanel("스캔 오류");
                    }
                    else
                    {
                        RenderResults();
                        _statusLabel.Text = string.Format("{0:HH:mm:ss} 스캔 완료", DateTime.Now);
                    }
                    SetBusy(false, null);
                }));
            });
            thread.IsBackground = true;
            thread.Start();
        }

        private void SetBusy(bool busy, string status)
        {
            _busy = busy;
            _refreshButton.Enabled = !busy;
            if (status != null)
            {
                _statusLabel.Text = status;
            }
            foreach (Control c in _transferPanel.Controls)
            {
                if (c is Button)
                {
                    c.Enabled = !busy;
                }
            }
        }

        private List<ScanResult> Switchable()
        {
            var list = new List<ScanResult>();
            foreach (var r in _allResults)
            {
                if (r.ChangeHostSupported && r.HostCount > 0)
                {
                    list.Add(r);
                }
            }
            return list;
        }

        private void RenderResults()
        {
            bool showAll = _showAllCheck.Checked;
            var shown = new List<ScanResult>();
            foreach (var r in _allResults)
            {
                if (showAll || r.ChangeHostSupported)
                {
                    shown.Add(r);
                }
            }

            _output.Text = FormatReport(shown, _allResults.Count, showAll);
            BuildTransferPanel();
        }

        private static string FormatReport(List<ScanResult> shown, int totalCount, bool showAll)
        {
            var sb = new StringBuilder();
            sb.AppendLine("LogiFlow HID++ 진단 결과");
            sb.AppendLine(new string('=', 60));
            if (showAll)
            {
                sb.AppendLine(string.Format("전체 응답 장치: {0}개 (진단 모드)", totalCount));
            }
            else
            {
                sb.AppendLine(string.Format("전환 가능 장치: {0}개  (전체 {1}개 중 · 빈 슬롯/리시버는 숨김)",
                    shown.Count, totalCount));
            }
            sb.AppendLine();

            if (shown.Count == 0)
            {
                sb.AppendLine("표시할 장치가 없습니다.");
                sb.AppendLine("로지텍 기기가 켜져 연결되어 있는지 확인하고 [새로 고침] 하세요.");
                sb.AppendLine("자세한 내용은 [로그 폴더 열기] 의 logiflow-diag.log 참조.");
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

        private void ClearTransferPanel(string message)
        {
            _transferPanel.Controls.Clear();
            _transferPanel.Controls.Add(new Label { AutoSize = true, Top = 8, Left = 8, Text = message });
        }

        private void BuildTransferPanel()
        {
            _transferPanel.Controls.Clear();
            var switchable = Switchable();

            if (switchable.Count == 0)
            {
                ClearTransferPanel("전환 가능한 기기가 없습니다. (0x1814 지원 기기 미탐지)");
                return;
            }

            int maxHost = 1;
            foreach (var d in switchable)
            {
                if (d.HostCount > maxHost)
                {
                    maxHost = d.HostCount;
                }
            }

            int y = 6;

            var title = new Label
            {
                AutoSize = true, Left = 6, Top = y,
                Font = new Font(Font, FontStyle.Bold),
                Text = string.Format("전환 테스트 — 전환 가능 기기 {0}개", switchable.Count),
            };
            _transferPanel.Controls.Add(title);
            y += 22;

            var warn = new Label
            {
                AutoSize = false, Left = 6, Top = y, Width = 720, Height = 32,
                ForeColor = Color.FromArgb(176, 0, 32),
                Text = "누르면 마우스·키보드가 해당 PC 로 넘어갑니다. 돌아오려면 그 PC 에서 채널 1 을 "
                     + "누르거나 기기의 Easy-Switch 버튼을 사용하세요.",
            };
            _transferPanel.Controls.Add(warn);
            y += 36;

            // 전체 전환 버튼(모든 전환 가능 기기를 같은 채널로).
            var allLabel = new Label { AutoSize = true, Left = 6, Top = y + 4, Text = "모두 함께:" };
            _transferPanel.Controls.Add(allLabel);
            int bx = 78;
            for (int h = 0; h < maxHost; h++)
            {
                int target = h;
                var b = new Button { Text = string.Format("채널 {0}로 전환", h + 1), Width = 104, Height = 26, Left = bx, Top = y };
                b.Click += (s, e) => DoSwitchAll(target);
                _transferPanel.Controls.Add(b);
                bx += 110;
            }
            y += 34;

            // 기기별 개별 전환.
            foreach (var d in switchable)
            {
                string cur = d.CurrentHost >= 0 ? (d.CurrentHost + 1).ToString() : "?";
                var devLabel = new Label
                {
                    AutoSize = true, Left = 6, Top = y + 4,
                    Text = string.Format("{0}  (현재 채널 {1})", d.DeviceName, cur),
                };
                _transferPanel.Controls.Add(devLabel);

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
        }

        private bool ConfirmSwitch(string what, int targetHost)
        {
            var result = MessageBox.Show(this,
                string.Format("{0} 를 채널 {1} 로 전환합니다.\r\n\r\n"
                    + "해당 PC 로 넘어가며, 이 PC 에서는 기기가 사라집니다.\r\n"
                    + "돌아오려면 그 PC 에서 채널 1 을 누르거나 기기의 Easy-Switch 버튼을 쓰세요.\r\n\r\n"
                    + "진행할까요?", what, targetHost + 1),
                "LogiFlow 전환 확인", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
            return result == DialogResult.Yes;
        }

        private void DoSwitchAll(int targetHost)
        {
            if (_busy)
            {
                return;
            }
            if (!ConfirmSwitch("전환 가능한 모든 기기", targetHost))
            {
                return;
            }
            RunSwitch(() =>
            {
                int sent = HostSwitcher.SwitchAll(Switchable(), targetHost, _log);
                return string.Format("채널 {0}로 전체 전환: {1}개 전송", targetHost + 1, sent);
            });
        }

        private void DoSwitchOne(ScanResult device, int targetHost)
        {
            if (_busy)
            {
                return;
            }
            if (!ConfirmSwitch(device.DeviceName, targetHost))
            {
                return;
            }
            RunSwitch(() =>
            {
                bool ok = HostSwitcher.SwitchOne(device, targetHost, _log);
                return string.Format("{0} → 채널 {1}: {2}", device.DeviceName, targetHost + 1,
                    ok ? "전송" : "실패(로그 확인)");
            });
        }

        private void RunSwitch(Func<string> action)
        {
            SetBusy(true, "전환 전송 중...");
            var thread = new Thread(() =>
            {
                string msg;
                try
                {
                    msg = action();
                }
                catch (Exception ex)
                {
                    msg = "전환 오류: " + ex.Message;
                    _log.LogError("전환 오류: " + ex);
                }
                BeginInvoke(new Action(() =>
                {
                    SetBusy(false, msg);
                    MessageBox.Show(this,
                        msg + "\r\n\r\n전환된 기기는 이 PC 에서 사라집니다. [새로 고침] 으로 현재 상태를 다시 읽을 수 있습니다.",
                        "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Information);
                }));
            });
            thread.IsBackground = true;
            thread.Start();
        }

        private void OpenLogFolder()
        {
            try
            {
                Directory.CreateDirectory(_log.LogDirectory);
                Process.Start("explorer.exe", "\"" + _log.LogDirectory + "\"");
            }
            catch (Exception ex)
            {
                MessageBox.Show(this, "로그 폴더를 열지 못했습니다:\r\n" + ex.Message,
                    "LogiFlow", MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }
    }
}
