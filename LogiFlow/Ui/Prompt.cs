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
}
