using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Handoff;

namespace LogiFlow
{
    /// <summary>
    /// 이 PC 의 모니터를 비율로 그리고, 다른 채널 PC 박스를 끌어다 모니터 변에 붙이는 캔버스.
    /// 붙인 변 = 그 채널로 넘어가는 엣지. 비어 있는(미배치) 채널은 엣지 전환 안 함.
    /// </summary>
    internal sealed class MonitorLayoutPanel : Panel
    {
        private sealed class ChannelBox
        {
            public int Channel;        // 0-base
            public bool Placed;
            public int MonitorIndex;
            public HandoffEdge Edge;
            public RectangleF Rect;    // 패널 픽셀 좌표(미배치/드래그 중 위치)
        }

        private const int BoxW = 104;
        private const int BoxH = 60;
        private const int Pad = 16;
        private const int SnapPx = 28;

        private readonly List<ChannelBox> _boxes = new List<ChannelBox>();
        private Screen[] _screens = new Screen[0];
        private readonly List<HandoffRect> _monitorPanelRects = new List<HandoffRect>();
        private int _currentChannel = -1;

        private ChannelBox _dragging;
        private PointF _dragOffset;

        public MonitorLayoutPanel()
        {
            DoubleBuffered = true;
            BackColor = Color.FromArgb(245, 246, 248);
            BorderStyle = BorderStyle.FixedSingle;
        }

        /// <summary>
        /// 캔버스 초기화. channels = 배치 대상 채널(0-base, 보통 current 제외),
        /// currentChannel = 이 PC 의 현재 채널, existing = 저장된 배치.
        /// </summary>
        public void SetData(IList<int> channels, int currentChannel, IList<EdgeAttachment> existing)
        {
            _currentChannel = currentChannel;
            _screens = Screen.AllScreens;
            _boxes.Clear();
            foreach (int ch in channels)
            {
                var box = new ChannelBox { Channel = ch, Placed = false };
                EdgeAttachment match = FindAttachment(existing, ch);
                if (match != null)
                {
                    int idx = IndexOfScreen(match.MonitorDeviceName);
                    if (idx >= 0)
                    {
                        box.Placed = true;
                        box.MonitorIndex = idx;
                        box.Edge = match.Edge;
                    }
                }
                _boxes.Add(box);
            }
            Invalidate();
        }

        private static EdgeAttachment FindAttachment(IList<EdgeAttachment> list, int channel)
        {
            if (list == null) return null;
            foreach (var a in list)
            {
                if (a.Channel == channel) return a;
            }
            return null;
        }

        private int IndexOfScreen(string deviceName)
        {
            for (int i = 0; i < _screens.Length; i++)
            {
                if (string.Equals(_screens[i].DeviceName, deviceName, StringComparison.Ordinal))
                {
                    return i;
                }
            }
            return -1;
        }

        /// <summary>현재 배치를 설정 저장용 목록으로 반환.</summary>
        public List<EdgeAttachment> GetAttachments()
        {
            var list = new List<EdgeAttachment>();
            foreach (var box in _boxes)
            {
                if (box.Placed && box.MonitorIndex >= 0 && box.MonitorIndex < _screens.Length)
                {
                    list.Add(new EdgeAttachment
                    {
                        Channel = box.Channel,
                        MonitorDeviceName = _screens[box.MonitorIndex].DeviceName,
                        Edge = box.Edge,
                    });
                }
            }
            return list;
        }

        private void RecomputeMonitorRects()
        {
            _monitorPanelRects.Clear();
            if (_screens.Length == 0) return;

            int vLeft = int.MaxValue, vTop = int.MaxValue, vRight = int.MinValue, vBottom = int.MinValue;
            foreach (Screen s in _screens)
            {
                Rectangle b = s.Bounds;
                vLeft = Math.Min(vLeft, b.Left);
                vTop = Math.Min(vTop, b.Top);
                vRight = Math.Max(vRight, b.Right);
                vBottom = Math.Max(vBottom, b.Bottom);
            }
            // 모니터 바깥(변에 붙는 박스)을 위한 여백을 가상 영역에 더한다.
            int vW = Math.Max(1, vRight - vLeft);
            int vH = Math.Max(1, vBottom - vTop);
            int marginX = (int)(vW * 0.35);
            int marginY = (int)(vH * 0.35);
            vLeft -= marginX; vRight += marginX;
            vTop -= marginY; vBottom += marginY;
            vW = Math.Max(1, vRight - vLeft);
            vH = Math.Max(1, vBottom - vTop);

            double scale = Math.Min(
                (ClientSize.Width - 2.0 * Pad) / vW,
                (ClientSize.Height - 2.0 * Pad) / vH);
            if (scale <= 0) scale = 0.01;

            double offX = Pad + (ClientSize.Width - 2.0 * Pad - vW * scale) / 2.0;
            double offY = Pad + (ClientSize.Height - 2.0 * Pad - vH * scale) / 2.0;

            foreach (Screen s in _screens)
            {
                Rectangle b = s.Bounds;
                int px = (int)(offX + (b.Left - vLeft) * scale);
                int py = (int)(offY + (b.Top - vTop) * scale);
                int pw = (int)(b.Width * scale);
                int ph = (int)(b.Height * scale);
                _monitorPanelRects.Add(new HandoffRect(px, py, pw, ph));
            }
        }

        private RectangleF BoxDisplayRect(ChannelBox box)
        {
            if (box == _dragging || !box.Placed)
            {
                if (box.Rect.Width > 0)
                {
                    return box.Rect;
                }
                // 미배치 기본 위치: 하단 트레이에 가로로 배열.
                int idx = _boxes.IndexOf(box);
                float x = Pad + idx * (BoxW + 10);
                float y = ClientSize.Height - BoxH - Pad;
                return new RectangleF(x, y, BoxW, BoxH);
            }
            if (box.MonitorIndex < 0 || box.MonitorIndex >= _monitorPanelRects.Count)
            {
                return new RectangleF(Pad, Pad, BoxW, BoxH);
            }
            HandoffRect snapped = HandoffLayout.SnapFlush(_monitorPanelRects[box.MonitorIndex], box.Edge, BoxW, BoxH);
            return new RectangleF(snapped.Left, snapped.Top, snapped.Width, snapped.Height);
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            RecomputeMonitorRects();
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;

            using (var header = new Font(Font, FontStyle.Bold))
            {
                string cur = _currentChannel >= 0
                    ? string.Format("이 PC = 채널 {0} (현재)", _currentChannel + 1)
                    : "이 PC";
                g.DrawString(cur, header, Brushes.DimGray, Pad, 2);
                g.DrawString("다른 채널 PC 박스를 모니터 변으로 끌어다 놓으세요", Font, Brushes.Gray, Pad, 20);
            }

            // 모니터.
            using (var monFill = new SolidBrush(Color.FromArgb(222, 228, 236)))
            using (var monPen = new Pen(Color.FromArgb(150, 160, 175), 2))
            {
                for (int i = 0; i < _monitorPanelRects.Count; i++)
                {
                    HandoffRect m = _monitorPanelRects[i];
                    var rect = new Rectangle(m.Left, m.Top, m.Width, m.Height);
                    g.FillRectangle(monFill, rect);
                    g.DrawRectangle(monPen, rect);
                    string label = "모니터 " + (i + 1);
                    SizeF sz = g.MeasureString(label, Font);
                    g.DrawString(label, Font, Brushes.DimGray,
                        m.Left + (m.Width - sz.Width) / 2, m.Top + (m.Height - sz.Height) / 2);
                }
            }

            // 채널 박스.
            using (var placedFill = new SolidBrush(Color.FromArgb(70, 130, 220)))
            using (var unplacedFill = new SolidBrush(Color.FromArgb(150, 160, 175)))
            using (var boxPen = new Pen(Color.FromArgb(40, 90, 170), 2))
            using (var white = new SolidBrush(Color.White))
            using (var boxFont = new Font(Font.FontFamily, 9f, FontStyle.Bold))
            {
                foreach (var box in _boxes)
                {
                    RectangleF r = BoxDisplayRect(box);
                    g.FillRectangle(box.Placed ? placedFill : unplacedFill, r);
                    g.DrawRectangle(boxPen, r.X, r.Y, r.Width, r.Height);
                    string label = string.Format("채널 {0} PC", box.Channel + 1);
                    SizeF sz = g.MeasureString(label, boxFont);
                    g.DrawString(label, boxFont, white,
                        r.X + (r.Width - sz.Width) / 2, r.Y + (r.Height - sz.Height) / 2);
                }
            }
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (_monitorPanelRects.Count == 0) RecomputeMonitorRects();
            for (int i = _boxes.Count - 1; i >= 0; i--)
            {
                RectangleF r = BoxDisplayRect(_boxes[i]);
                if (r.Contains(e.Location))
                {
                    _dragging = _boxes[i];
                    _dragOffset = new PointF(e.X - r.X, e.Y - r.Y);
                    _dragging.Rect = r;
                    Invalidate();
                    return;
                }
            }
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            if (_dragging == null) return;
            _dragging.Rect = new RectangleF(e.X - _dragOffset.X, e.Y - _dragOffset.Y, BoxW, BoxH);
            Invalidate();
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            if (_dragging == null) return;
            if (_monitorPanelRects.Count == 0) RecomputeMonitorRects();

            var boxRect = new HandoffRect(
                (int)_dragging.Rect.X, (int)_dragging.Rect.Y, BoxW, BoxH);
            int monIdx;
            HandoffEdge edge;
            if (HandoffLayout.ResolveAttachment(_monitorPanelRects, boxRect, SnapPx, out monIdx, out edge))
            {
                _dragging.Placed = true;
                _dragging.MonitorIndex = monIdx;
                _dragging.Edge = edge;
                _dragging.Rect = RectangleF.Empty; // 배치되면 SnapFlush 위치 사용
            }
            else
            {
                // 변에서 멀면 미배치(엣지 전환 제외).
                _dragging.Placed = false;
            }
            _dragging = null;
            Invalidate();
        }
    }
}
