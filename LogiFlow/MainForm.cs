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
    /// 읽기 전용 진단 창. 로지텍 HID++ 장치를 스캔해 결과를 보여준다.
    /// 전환·엣지·트레이 기능은 이번 판에 없다(읽기만).
    /// </summary>
    internal sealed class MainForm : Form
    {
        private readonly DiagLogger _log;
        private Button _refreshButton;
        private Button _openLogButton;
        private Label _statusLabel;
        private TextBox _output;

        public MainForm(DiagLogger log)
        {
            _log = log;
            BuildUi();
        }

        private void BuildUi()
        {
            Text = "LogiFlow — HID++ 읽기 전용 진단";
            Width = 760;
            Height = 560;
            MinimumSize = new Size(560, 400);
            StartPosition = FormStartPosition.CenterScreen;
            Font = new Font("Segoe UI", 9f);

            var topPanel = new Panel
            {
                Dock = DockStyle.Top,
                Height = 44,
                Padding = new Padding(8),
            };

            _refreshButton = new Button
            {
                Text = "새로 고침",
                Width = 100,
                Height = 28,
                Left = 8,
                Top = 8,
            };
            _refreshButton.Click += (s, e) => StartScan();

            _openLogButton = new Button
            {
                Text = "로그 폴더 열기",
                Width = 120,
                Height = 28,
                Left = 116,
                Top = 8,
            };
            _openLogButton.Click += (s, e) => OpenLogFolder();

            _statusLabel = new Label
            {
                AutoSize = true,
                Left = 248,
                Top = 14,
                Text = "준비됨",
            };

            topPanel.Controls.Add(_refreshButton);
            topPanel.Controls.Add(_openLogButton);
            topPanel.Controls.Add(_statusLabel);

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

            Shown += (s, e) => StartScan();
        }

        private void StartScan()
        {
            _refreshButton.Enabled = false;
            _statusLabel.Text = "스캔 중...";
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

                string report = error != null
                    ? "스캔 중 오류가 발생했습니다.\r\n\r\n" + error
                    : FormatReport(results);

                BeginInvoke(new Action(() =>
                {
                    _output.Text = report;
                    _statusLabel.Text = error != null
                        ? "오류 — 로그 확인"
                        : string.Format("장치 {0}개 · {1:HH:mm:ss}", results.Count, DateTime.Now);
                    _refreshButton.Enabled = true;
                }));
            });
            thread.IsBackground = true;
            thread.Start();
        }

        private static string FormatReport(List<ScanResult> results)
        {
            var sb = new StringBuilder();
            sb.AppendLine("LogiFlow 읽기 전용 진단 결과");
            sb.AppendLine(new string('=', 60));
            sb.AppendLine(string.Format("응답한 HID++ 장치: {0}개", results.Count));
            sb.AppendLine();

            if (results.Count == 0)
            {
                sb.AppendLine("응답한 장치가 없습니다.");
                sb.AppendLine();
                sb.AppendLine("확인 사항:");
                sb.AppendLine("  - 로지텍 기기(MX Master 4 등)가 켜져 연결되어 있는지");
                sb.AppendLine("  - 로지텍 Options+ 등 다른 프로그램이 점유 중인지");
                sb.AppendLine("  - 그래도 안 되면 [로그 폴더 열기] 의 logiflow-diag.log 를 보내 주세요.");
                return sb.ToString();
            }

            int n = 0;
            foreach (var r in results)
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
                    else
                    {
                        sb.AppendLine("    호스트 개수    (조회 실패)");
                    }
                }
                else
                {
                    sb.AppendLine("    0x1814 인덱스  미지원");
                }
                sb.AppendLine(string.Format("    경로          {0}", r.Path));
                if (!string.IsNullOrEmpty(r.Note))
                {
                    sb.AppendLine(string.Format("    비고          {0}", r.Note));
                }
                sb.AppendLine();
            }

            sb.AppendLine(new string('-', 60));
            sb.AppendLine("* 이번 판은 읽기 전용입니다. 호스트 전환(쓰기)은 수행하지 않습니다.");
            return sb.ToString();
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
