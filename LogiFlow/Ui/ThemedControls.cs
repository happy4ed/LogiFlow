using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace LogiFlow.Ui
{
    public delegate void IconPaint(Graphics g, RectangleF r, Color c);

    public enum ButtonVariant { Default, Primary, Ghost, Icon }

    internal static class UiPaint
    {
        public static GraphicsPath RoundRect(RectangleF r, float radius)
        {
            float d = radius * 2;
            var path = new GraphicsPath();
            if (d <= 0)
            {
                path.AddRectangle(r);
                return path;
            }
            path.AddArc(r.X, r.Y, d, d, 180, 90);
            path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
            path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
            path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
            path.CloseFigure();
            return path;
        }
    }

    /// <summary>macOS풍 라운드 버튼(LocalDex ThemedButton 식 커스텀 페인팅). 변형: 기본/주요/고스트/아이콘.</summary>
    public sealed class ThemedButton : Button
    {
        private bool _hover;
        public ButtonVariant Variant = ButtonVariant.Default;
        public IconPaint Icon;

        public ThemedButton()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            FlatStyle = FlatStyle.Flat;
            FlatAppearance.BorderSize = 0;
            BackColor = Color.Transparent;
            Height = 28;
            Cursor = Cursors.Hand;
            Theme.Changed += OnTheme;
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }
        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Content);

            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            Color bg, border, fg;
            switch (Variant)
            {
                case ButtonVariant.Primary:
                    bg = Theme.Accent; border = Theme.Accent; fg = Color.White; break;
                case ButtonVariant.Ghost:
                    bg = Color.Transparent; border = Color.Transparent; fg = Theme.Accent; break;
                case ButtonVariant.Icon:
                    bg = _hover ? Theme.Hover : Theme.Section; border = Theme.Border; fg = Theme.Muted; break;
                default:
                    bg = _hover ? Theme.Hover : Theme.Section; border = Theme.Border; fg = Theme.Ink; break;
            }

            using (var path = UiPaint.RoundRect(r, 7f))
            {
                if (bg.A > 0)
                {
                    using (var b = new SolidBrush(bg)) g.FillPath(b, path);
                }
                if (border.A > 0)
                {
                    using (var p = new Pen(border, 1f)) g.DrawPath(p, path);
                }
            }

            // 아이콘 + 텍스트 배치.
            float iconSize = 14f;
            bool hasText = !string.IsNullOrEmpty(Text);
            bool hasIcon = Icon != null;
            using (var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
            using (var tb = new SolidBrush(fg))
            {
                if (hasIcon && !hasText)
                {
                    var ir = new RectangleF((Width - iconSize) / 2f, (Height - iconSize) / 2f, iconSize, iconSize);
                    Icon(g, ir, fg);
                }
                else if (hasIcon)
                {
                    SizeF ts = g.MeasureString(Text, Font);
                    float total = iconSize + 6 + ts.Width;
                    float x = (Width - total) / 2f;
                    var ir = new RectangleF(x, (Height - iconSize) / 2f, iconSize, iconSize);
                    Icon(g, ir, fg);
                    var tr = new RectangleF(x + iconSize + 6, 0, ts.Width + 2, Height);
                    using (var lf = new StringFormat { Alignment = StringAlignment.Near, LineAlignment = StringAlignment.Center })
                        g.DrawString(Text, Font, tb, tr, lf);
                }
                else if (hasText)
                {
                    g.DrawString(Text, Font, tb, new RectangleF(0, 0, Width, Height), sf);
                }
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>28x16 토글 스위치.</summary>
    public sealed class ThemedToggle : Control
    {
        private bool _checked;
        public event EventHandler CheckedChanged;

        public ThemedToggle()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Size = new Size(28, 16);
            Cursor = Cursors.Hand;
            Theme.Changed += OnTheme;
        }

        public bool Checked
        {
            get { return _checked; }
            set { if (_checked != value) { _checked = value; Invalidate(); var h = CheckedChanged; if (h != null) h(this, EventArgs.Empty); } }
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }
        protected override void OnClick(EventArgs e) { Checked = !Checked; base.OnClick(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Content);
            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            using (var path = UiPaint.RoundRect(r, r.Height / 2f))
            using (var b = new SolidBrush(_checked ? Theme.Accent : Theme.ToggleOff))
                g.FillPath(b, path);
            float d = Height - 4;
            float x = _checked ? Width - d - 2 : 2;
            using (var tb = new SolidBrush(Color.White))
                g.FillEllipse(tb, x, 2, d, d);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>세그먼트 선택(시스템/라이트/다크 등).</summary>
    public sealed class SegmentedControl : Control
    {
        private string[] _items = new string[0];
        private int _selected;
        public event EventHandler SelectedChanged;

        public SegmentedControl()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Height = 30;
            Cursor = Cursors.Hand;
            Theme.Changed += OnTheme;
        }

        public void SetItems(string[] items, int selected)
        {
            _items = items ?? new string[0];
            _selected = selected;
            Invalidate();
        }

        public int SelectedIndex
        {
            get { return _selected; }
            set { if (_selected != value) { _selected = value; Invalidate(); var h = SelectedChanged; if (h != null) h(this, EventArgs.Empty); } }
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            if (_items.Length > 0)
            {
                int w = Width / _items.Length;
                int idx = Math.Min(_items.Length - 1, Math.Max(0, e.X / Math.Max(1, w)));
                SelectedIndex = idx;
            }
            base.OnMouseDown(e);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Content);
            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            using (var path = UiPaint.RoundRect(r, 8f))
            using (var b = new SolidBrush(Theme.Chip))
                g.FillPath(b, path);

            if (_items.Length == 0) return;
            float segW = (float)Width / _items.Length;
            using (var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
            {
                for (int i = 0; i < _items.Length; i++)
                {
                    var seg = new RectangleF(i * segW + 3, 3, segW - 6, Height - 6);
                    if (i == _selected)
                    {
                        using (var sp = UiPaint.RoundRect(seg, 6f))
                        using (var sb = new SolidBrush(Theme.Section))
                            g.FillPath(sb, sp);
                    }
                    using (var tb = new SolidBrush(i == _selected ? Theme.Ink : Theme.Muted))
                        g.DrawString(_items[i], Font, tb, new RectangleF(i * segW, 0, segW, Height), sf);
                }
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>사이드바 내비 항목(아이콘+라벨, active 강조).</summary>
    public sealed class NavButton : Control
    {
        private bool _active;
        private bool _hover;
        public IconPaint Icon;
        public event EventHandler Selected;

        public NavButton(string text, IconPaint icon)
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Text = text;
            Icon = icon;
            Height = 36;
            Cursor = Cursors.Hand;
            Theme.Changed += OnTheme;
        }

        public bool Active { get { return _active; } set { _active = value; Invalidate(); } }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }
        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnClick(EventArgs e) { var h = Selected; if (h != null) h(this, EventArgs.Empty); base.OnClick(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Sidebar);
            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            if (_active)
            {
                using (var path = UiPaint.RoundRect(r, 8f))
                using (var b = new SolidBrush(Theme.AccentSoft))
                    g.FillPath(b, path);
            }
            else if (_hover)
            {
                using (var path = UiPaint.RoundRect(r, 8f))
                using (var b = new SolidBrush(Theme.Hover))
                    g.FillPath(b, path);
            }

            Color fg = _active ? Theme.Accent : Theme.Ink;
            Color ic = _active ? Theme.Accent : Theme.Muted;
            if (Icon != null)
            {
                Icon(g, new RectangleF(10, (Height - 18) / 2f, 18, 18), ic);
            }
            using (var tb = new SolidBrush(fg))
            using (var sf = new StringFormat { LineAlignment = StringAlignment.Center })
                g.DrawString(Text, Font, tb, new RectangleF(36, 0, Width - 38, Height), sf);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>라운드 카드(Section 배경 + Border). 자식은 절대 배치.</summary>
    public sealed class ThemedCard : Panel
    {
        public ThemedCard()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            BackColor = Theme.Section;
            Theme.Changed += OnTheme;
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) { BackColor = Theme.Section; UpdateRegion(); Invalidate(); } }
        protected override void OnResize(EventArgs e) { base.OnResize(e); UpdateRegion(); }
        protected override void OnHandleCreated(EventArgs e) { base.OnHandleCreated(e); UpdateRegion(); }

        private void UpdateRegion()
        {
            if (Width <= 2 || Height <= 2) return;
            using (var path = UiPaint.RoundRect(new RectangleF(0, 0, Width, Height), 10f))
                Region = new Region(path);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            using (var path = UiPaint.RoundRect(r, 10f))
            using (var pen = new Pen(Theme.Border, 1f))
                e.Graphics.DrawPath(pen, path);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>알약형 배지(채널·상태). UseAccent=true 면 accent-soft 강조.</summary>
    public sealed class ThemedBadge : Control
    {
        public bool AutoWidth = true;
        public bool UseAccent;

        public ThemedBadge()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw
                     | ControlStyles.SupportsTransparentBackColor, true);
            BackColor = Color.Transparent;
            Height = 22;
            Font = new Font("Segoe UI", 8.5f, FontStyle.Bold);
            Theme.Changed += OnTheme;
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }

        protected override void OnTextChanged(EventArgs e) { base.OnTextChanged(e); Relayout(); Invalidate(); }
        protected override void OnFontChanged(EventArgs e) { base.OnFontChanged(e); Relayout(); }

        private void Relayout()
        {
            if (!AutoWidth) return;
            Size sz = TextRenderer.MeasureText(Text ?? "", Font);
            Width = sz.Width + 20;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            var r = new RectangleF(0.5f, 0.5f, Width - 1.5f, Height - 1.5f);
            Color bg = UseAccent ? Theme.AccentSoft : Theme.Chip;
            Color fg = UseAccent ? Theme.Accent : Theme.Muted;
            using (var path = UiPaint.RoundRect(r, Height / 2f))
            using (var b = new SolidBrush(bg))
                g.FillPath(b, path);
            using (var tb = new SolidBrush(fg))
            using (var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
                g.DrawString(Text, Font, tb, new RectangleF(0, 0, Width, Height), sf);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }

    /// <summary>작은 아이콘 표시용 컨트롤.</summary>
    public sealed class IconView : Control
    {
        public IconPaint Icon;
        public Color? Tint;

        public IconView(IconPaint icon, int size)
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor, true);
            BackColor = Color.Transparent;
            Icon = icon;
            Size = new Size(size, size);
            Theme.Changed += OnTheme;
        }

        private void OnTheme(object s, EventArgs e) { if (!IsDisposed) Invalidate(); }

        protected override void OnPaint(PaintEventArgs e)
        {
            e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
            if (Icon != null)
            {
                Icon(e.Graphics, new RectangleF(0, 0, Width, Height), Tint ?? Theme.Muted);
            }
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing) Theme.Changed -= OnTheme;
            base.Dispose(disposing);
        }
    }
}
