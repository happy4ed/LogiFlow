using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace LogiFlow.Ui
{
    /// <summary>테마 색의 얇은 세로 스크롤바(네이티브 OS 스크롤바 대체).</summary>
    public sealed class ThemedScrollBar : Control
    {
        private int _content = 1;
        private int _viewport = 1;
        private int _offset;
        private bool _dragging;
        private int _dragStartY;
        private int _dragStartOffset;
        private bool _hover;

        public event Action<int> Scrolled;

        public ThemedScrollBar()
        {
            SetStyle(ControlStyles.UserPaint | ControlStyles.AllPaintingInWmPaint
                     | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            Width = 10;
            Theme.Changed += (s, e) => { if (!IsDisposed) Invalidate(); };
        }

        public void SetRange(int content, int viewport, int offset)
        {
            _content = Math.Max(1, content);
            _viewport = Math.Max(1, viewport);
            _offset = offset;
            Invalidate();
        }

        private int MaxOffset { get { return Math.Max(0, _content - _viewport); } }

        private Rectangle ThumbRect()
        {
            if (_content <= _viewport) return Rectangle.Empty;
            int track = Height - 4;
            int th = Math.Max(28, (int)((double)_viewport / _content * track));
            int maxThumbTop = track - th;
            int top = MaxOffset > 0 ? (int)((double)_offset / MaxOffset * maxThumbTop) : 0;
            return new Rectangle(2, 2 + top, Width - 4, th);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Parent != null ? Parent.BackColor : Theme.Content);
            Rectangle thumb = ThumbRect();
            if (thumb.IsEmpty) return;
            Color c = Theme.Blend(Theme.Muted, Theme.Content, _hover || _dragging ? 0.55 : 0.35);
            using (var path = UiPaint.RoundRect(thumb, (Width - 4) / 2f))
            using (var b = new SolidBrush(c))
                g.FillPath(b, path);
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            Rectangle thumb = ThumbRect();
            if (thumb.Contains(e.Location))
            {
                _dragging = true; _dragStartY = e.Y; _dragStartOffset = _offset;
            }
            else if (!thumb.IsEmpty)
            {
                // 트랙 클릭: 한 페이지 이동.
                int dir = e.Y < thumb.Top ? -1 : 1;
                Apply(_offset + dir * (_viewport - 40));
            }
            base.OnMouseDown(e);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            if (_dragging)
            {
                int track = Height - 4;
                Rectangle thumb = ThumbRect();
                int maxThumbTop = Math.Max(1, track - thumb.Height);
                int dy = e.Y - _dragStartY;
                int deltaOffset = (int)((double)dy / maxThumbTop * MaxOffset);
                Apply(_dragStartOffset + deltaOffset);
            }
            base.OnMouseMove(e);
        }

        protected override void OnMouseUp(MouseEventArgs e) { _dragging = false; base.OnMouseUp(e); }
        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }

        private void Apply(int off)
        {
            off = Math.Max(0, Math.Min(MaxOffset, off));
            if (off != _offset)
            {
                _offset = off;
                Invalidate();
                var h = Scrolled;
                if (h != null) h(off);
            }
            else Invalidate();
        }
    }

    /// <summary>
    /// 세로 전용 테마 스크롤 호스트. 네이티브 AutoScroll 대신 커스텀 스크롤바를 쓰고,
    /// 콘텐츠 폭을 뷰포트 폭(스크롤바 자리 제외)에 맞춰 **가로 스크롤이 절대 생기지 않게** 한다.
    /// </summary>
    public sealed class ThemedScrollPanel : Panel, IMessageFilter
    {
        private const int BarW = 10;
        private const int WM_MOUSEWHEEL = 0x020A;
        private readonly ThemedScrollBar _bar;
        private Control _content;
        private int _offset;
        private int _lastViewportWidth = -1;

        /// <summary>뷰포트 폭(= 콘텐츠가 차지할 폭). 항상 스크롤바 자리를 예약한다.</summary>
        public int ViewportWidth { get { return Math.Max(1, Width - BarW); } }

        /// <summary>뷰포트 폭이 바뀌면 발생(소유자가 콘텐츠를 그 폭으로 다시 빌드).</summary>
        public event EventHandler ViewportWidthChanged;

        public ThemedScrollPanel()
        {
            SetStyle(ControlStyles.OptimizedDoubleBuffer, true);
            _bar = new ThemedScrollBar();
            _bar.Scrolled += (off) => { _offset = off; Relayout(false); };
            Controls.Add(_bar);
            Application.AddMessageFilter(this);
        }

        // 포커스와 무관하게 커서가 이 패널 위에 있으면 휠로 세로 스크롤.
        public bool PreFilterMessage(ref Message m)
        {
            if (m.Msg != WM_MOUSEWHEEL || !Visible || !IsHandleCreated) return false;
            try
            {
                Point local = PointToClient(Cursor.Position);
                if (!ClientRectangle.Contains(local)) return false;
                int delta = (short)((m.WParam.ToInt64() >> 16) & 0xffff);
                int ch = _content != null ? _content.Height : 0;
                int maxOff = Math.Max(0, ch - Height);
                if (maxOff <= 0) return false;
                _offset = Math.Max(0, Math.Min(maxOff, _offset - delta));
                Relayout(false);
                return true;
            }
            catch
            {
                return false;
            }
        }

        public void SetContent(Control content)
        {
            if (_content != null) { Controls.Remove(_content); _content.Dispose(); }
            _content = content;
            if (_content != null)
            {
                _content.Width = ViewportWidth;
                _content.Left = 0;
                _content.Top = 0;
                Controls.Add(_content);
                _content.BringToFront();
                _bar.BringToFront();
            }
            _offset = 0;
            Relayout(false);
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            Relayout(true);
        }

        protected override void Dispose(bool disposing)
        {
            if (disposing)
            {
                Application.RemoveMessageFilter(this);
            }
            base.Dispose(disposing);
        }

        private void Relayout(bool fromResize)
        {
            _bar.SetBounds(Width - BarW, 0, BarW, Height);
            int ch = _content != null ? _content.Height : 0;
            int maxOff = Math.Max(0, ch - Height);
            _offset = Math.Max(0, Math.Min(maxOff, _offset));

            bool needBar = ch > Height;
            _bar.Visible = needBar;
            _bar.SetRange(ch, Height, _offset);

            if (_content != null)
            {
                if (_content.Width != ViewportWidth) _content.Width = ViewportWidth;
                _content.Top = -_offset;
                _content.Left = 0;
            }

            if (fromResize && ViewportWidth != _lastViewportWidth)
            {
                _lastViewportWidth = ViewportWidth;
                var h = ViewportWidthChanged;
                if (h != null) h(this, EventArgs.Empty);
            }
        }
    }
}
