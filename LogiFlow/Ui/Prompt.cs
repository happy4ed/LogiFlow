using System;
using System.Drawing;
using System.Windows.Forms;

namespace LogiFlow.Ui
{
    /// <summary>간단한 한 줄 입력 대화상자(별칭 편집 등).</summary>
    public static class Prompt
    {
        public static string Show(IWin32Window owner, string title, string label, string initial)
        {
            using (var form = new Form())
            {
                form.Text = title;
                form.FormBorderStyle = FormBorderStyle.FixedDialog;
                form.StartPosition = FormStartPosition.CenterParent;
                form.MinimizeBox = false;
                form.MaximizeBox = false;
                form.ClientSize = new Size(340, 130);
                form.BackColor = Theme.Content;
                form.ForeColor = Theme.Ink;

                var lab = new Label { Text = label, Left = 16, Top = 16, Width = 308, ForeColor = Theme.Ink, AutoSize = false, Height = 20 };
                var box = new TextBox { Left = 16, Top = 42, Width = 308, Text = initial ?? "" };
                var ok = new Button { Text = "확인", DialogResult = DialogResult.OK, Left = 160, Top = 82, Width = 76 };
                var cancel = new Button { Text = "취소", DialogResult = DialogResult.Cancel, Left = 248, Top = 82, Width = 76 };

                form.Controls.Add(lab);
                form.Controls.Add(box);
                form.Controls.Add(ok);
                form.Controls.Add(cancel);
                form.AcceptButton = ok;
                form.CancelButton = cancel;

                box.SelectAll();
                box.Focus();

                return form.ShowDialog(owner) == DialogResult.OK ? box.Text.Trim() : null;
            }
        }
    }

    /// <summary>
    /// 오래 걸리는 동작용 모달 — 단계 메시지 + 진행 막대. 모달이라 처리 중 다른 조작을 막아, 눌러도
    /// 반응 없어 버그처럼 보이던 것을 방지한다. 백그라운드 스레드에서 SetStep/Finish 를 호출하고(Invoke 안전),
    /// 메인 스레드는 ShowDialog 로 대기한다.
    /// Finish 를 부르면 같은 창에서 진행 막대가 사라지고 본문이 결과 메시지로 바뀌며 [확인] 버튼이 나타난다
    /// (팝업이 닫힌 뒤 또 다른 팝업이 뜨지 않게 — 한 창에서 진행→결과→확인).
    /// </summary>
    public sealed class ProgressModal : Form
    {
        private readonly Label _step;
        private readonly ProgressBar _bar;
        private readonly ThemedButton _ok;

        public ProgressModal(string title)
        {
            Text = title;
            FormBorderStyle = FormBorderStyle.FixedDialog;
            StartPosition = FormStartPosition.CenterParent;
            ControlBox = false;
            MinimizeBox = false;
            MaximizeBox = false;
            ShowInTaskbar = false;
            ClientSize = new Size(440, 244);
            BackColor = Theme.Content;
            ForeColor = Theme.Ink;

            var head = new Label { Text = title, Left = 20, Top = 16, Width = 400, Height = 24, AutoSize = false, Font = UiFont.Get(10.5f, FontStyle.Bold), ForeColor = Theme.Ink };
            _step = new Label { Text = "준비 중…", Left = 20, Top = 48, Width = 400, Height = 150, AutoSize = false, Font = UiFont.Get(9f), ForeColor = Theme.Muted };
            _bar = new ProgressBar { Left = 20, Top = 210, Width = 400, Height = 10, Style = ProgressBarStyle.Marquee, MarqueeAnimationSpeed = 30 };
            _ok = new ThemedButton { Text = "확인", Left = 330, Top = 202, Width = 90, Height = 32, Visible = false, DialogResult = DialogResult.OK };

            Controls.Add(head);
            Controls.Add(_step);
            Controls.Add(_bar);
            Controls.Add(_ok);
        }

        public void SetStep(string s)
        {
            try
            {
                if (!IsHandleCreated) return;
                BeginInvoke((Action)(() => { try { _step.Text = s; } catch { } }));
            }
            catch { }
        }

        /// <summary>처리 완료 — 진행 막대를 감추고 본문을 결과로 바꾼 뒤 [확인] 버튼을 노출한다.</summary>
        public void Finish(string resultText, bool error)
        {
            try
            {
                if (!IsHandleCreated) return;
                BeginInvoke((Action)(() =>
                {
                    try
                    {
                        _bar.Visible = false;
                        _step.Text = resultText;
                        _step.ForeColor = Theme.Ink;
                        _ok.Visible = true;
                        AcceptButton = _ok;
                        _ok.Focus();
                    }
                    catch { }
                }));
            }
            catch { }
        }
    }

    /// <summary>오픈소스 라이선스(MIT) 보기 — 읽기 전용, 테마 스크롤(ThemedScrollPanel) 사용.</summary>
    public static class LicenseDialog
    {
        private const string Body =
            "LogiFlow\r\n" +
            "로지텍 Easy-Switch 모니터 가장자리 자동 전환\r\n\r\n" +
            "MIT License\r\n\r\n" +
            "Copyright (c) 2026 happy4ed\r\n\r\n" +
            "Permission is hereby granted, free of charge, to any person obtaining a copy " +
            "of this software and associated documentation files (the \"Software\"), to deal " +
            "in the Software without restriction, including without limitation the rights " +
            "to use, copy, modify, merge, publish, distribute, sublicense, and/or sell " +
            "copies of the Software, and to permit persons to whom the Software is " +
            "furnished to do so, subject to the following conditions:\r\n\r\n" +
            "The above copyright notice and this permission notice shall be included in all " +
            "copies or substantial portions of the Software.\r\n\r\n" +
            "THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR " +
            "IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, " +
            "FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE " +
            "AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER " +
            "LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, " +
            "OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE " +
            "SOFTWARE.\r\n\r\n" +
            "─────────────────────────────\r\n" +
            "구성 / 사용 기술\r\n" +
            "• .NET Framework 4.6.2 · WinForms (Microsoft)\r\n" +
            "• Windows HID / SetupAPI (P/Invoke) — Logitech HID++ 2.0 (feature 0x1814 Change Host)\r\n" +
            "• 설치 패키지: Inno Setup\r\n";

        public static void Show(IWin32Window owner)
        {
            using (var form = new Form())
            {
                form.Text = "오픈소스 라이선스";
                form.FormBorderStyle = FormBorderStyle.FixedDialog;
                form.StartPosition = FormStartPosition.CenterParent;
                form.MinimizeBox = false;
                form.MaximizeBox = false;
                form.ClientSize = new Size(560, 460);
                form.BackColor = Theme.Content;
                form.ForeColor = Theme.Ink;

                var pad = new Panel { Dock = DockStyle.Fill, Padding = new Padding(16, 16, 16, 8), BackColor = Theme.Content };
                var scroll = new ThemedScrollPanel { Dock = DockStyle.Fill, BackColor = Theme.Field };
                pad.Controls.Add(scroll);

                var btnBar = new Panel { Dock = DockStyle.Bottom, Height = 48, BackColor = Theme.Content };
                var close = new ThemedButton { Text = "닫기", Width = 88, Height = 28, Top = 10, Anchor = AnchorStyles.Right | AnchorStyles.Top };
                close.Left = form.ClientSize.Width - 104;
                close.Click += (s, e) => form.Close();
                btnBar.Controls.Add(close);

                form.Controls.Add(pad);
                form.Controls.Add(btnBar);
                form.AcceptButton = close;

                // 레이아웃이 잡힌 뒤(뷰포트 폭 확정) 본문 라벨을 측정해 넣는다 → 테마 스크롤바가 콘텐츠에 맞게 뜬다.
                form.Shown += (s, e) =>
                {
                    try
                    {
                        var font = UiFont.Get(9f);
                        int w = scroll.ViewportWidth;
                        int textW = Math.Max(1, w - 12);
                        int h = TextRenderer.MeasureText(Body, font, new Size(textW, int.MaxValue),
                            TextFormatFlags.WordBreak | TextFormatFlags.TextBoxControl).Height + 16;
                        var lab = new Label
                        {
                            AutoSize = false,
                            Width = w,
                            Height = h,
                            Padding = new Padding(6, 6, 6, 6),
                            BackColor = Theme.Field,
                            ForeColor = Theme.Ink,
                            Font = font,
                            Text = Body,
                        };
                        scroll.SetContent(lab);
                    }
                    catch { }
                };

                form.ShowDialog(owner);
            }
        }
    }
}
