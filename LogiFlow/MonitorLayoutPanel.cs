using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Handoff;
using LogiFlow.Ui;

namespace LogiFlow
{
    /// <summary>
    /// 전환 관리 캔버스. 이 PC 모니터를 실제 배치로 그리고, 다른 채널 PC 박스(모니터 크기 사각형)를
    /// 변에 끌어 붙인다. 접촉=자동(파랑 실선) · 분리=수동(점선 회색). 승인 시안 switch 화면 구현.
    /// </summary>
    internal sealed class MonitorLayoutPanel : Panel
    {
        private sealed class ChannelBox
        {
            public int Channel;        // 0-base
            public bool Placed;
            public int MonitorIndex;
            public HandoffEdge Edge;
            public RectangleF Rect;    // 패널 픽셀(미배치/드래그 중)
        }

        private const int Pad = 14;
        private const int SnapPx = 30;

        private readonly List<ChannelBox> _boxes = new List<ChannelBox>();
        private Screen[] _screens = new Screen[0];
        private readonly List<HandoffRect> _monitorPanelRects = new List<HandoffRect>();
        private int _currentChannel = -1;
        private int _primaryIndex = 0;
        private Func<int, string> _aliasFor = (ch) => "채널 " + (ch + 1);
        private float _boxW = 120, _boxH = 72;
        // 고정 스케일 배치용: 가상 경계(여백 포함)와 px/가상 스케일. 콘텐츠가 뷰포트보다 크면 호스트가 스크롤.
        private int _vLeft, _vTop, _vW = 1, _vH = 1;
        private double _scale = 0.1;

        private ChannelBox _dragging;
        private PointF _dragOffset;

        public event EventHandler AttachmentsChanged;

        public MonitorLayoutPanel()
        {
            DoubleBuffered = true;
            SetStyle(ControlStyles.ResizeRedraw, true);
            Theme.Changed += (s, e) => { if (!IsDisposed) Invalidate(); };
        }

        public void SetData(IList<int> channels, int currentChannel, IList<EdgeAttachment> existing, Func<int, string> aliasFor)
        {
            _currentChannel = currentChannel;
            _aliasFor = aliasFor ?? _aliasFor;
            _screens = Screen.AllScreens;
            _primaryIndex = 0;
            for (int i = 0; i < _screens.Length; i++)
            {
                if (_screens[i].Primary) { _primaryIndex = i; break; }
            }
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
            ComputeVirtual();
            Invalidate();
        }

        // 스케일 한계: 평소(모니터 1~3개)엔 뷰포트에 꼭 맞고, 아주 많아질 때만 최소 스케일로 내려가 스크롤된다.
        private const double MinScale = 0.035;  // 이보다 작아지면 축소 대신 스크롤
        private const double MaxScale = 0.16;   // 모니터 1개가 pane 을 꽉 채우지 않게

        /// <summary>모니터 가상 경계(채널 박스가 변에 붙을 여백 포함)만 계산. 스케일은 RelayoutToHost 에서 뷰포트 기준.</summary>
        private void ComputeVirtual()
        {
            if (_screens.Length == 0) { _vLeft = _vTop = 0; _vW = _vH = 1; return; }

            int vLeft = int.MaxValue, vTop = int.MaxValue, vRight = int.MinValue, vBottom = int.MinValue;
            double sumWo = 0, sumHo = 0;
            foreach (Screen s in _screens)
            {
                Rectangle b = s.Bounds;
                vLeft = Math.Min(vLeft, b.Left); vTop = Math.Min(vTop, b.Top);
                vRight = Math.Max(vRight, b.Right); vBottom = Math.Max(vBottom, b.Bottom);
                sumWo += b.Width; sumHo += b.Height;
            }
            int n = _screens.Length;
            double avgWo = sumWo / n, avgHo = sumHo / n;
            // 변에 붙는 채널 박스(=모니터 1칸)와 라벨·미배치 트레이가 들어갈 여백(가로 1.2칸·세로 1.5칸).
            int marginX = (int)(avgWo * 1.2), marginY = (int)(avgHo * 1.5);
            vLeft -= marginX; vRight += marginX; vTop -= marginY; vBottom += marginY;
            _vLeft = vLeft; _vTop = vTop;
            _vW = Math.Max(1, vRight - vLeft);
            _vH = Math.Max(1, vBottom - vTop);
        }

        /// <summary>
        /// 호스트(AutoScroll 패널) 뷰포트에 '꼭 맞는' 스케일을 구한다(평소 스크롤 없음). 다만 최소 스케일로
        /// 바닥을 두어, 모니터가 아주 많아 그보다 작아져야 할 때만 콘텐츠가 뷰포트보다 커져 스크롤된다.
        /// </summary>
        public void RelayoutToHost(Size viewport)
        {
            if (_screens.Length == 0)
            {
                Size = new Size(Math.Max(1, viewport.Width), Math.Max(1, viewport.Height));
                Invalidate();
                return;
            }
            ComputeVirtual();
            double fitW = (viewport.Width - 2.0 * Pad) / _vW;
            double fitH = (viewport.Height - 2.0 * Pad) / _vH;
            double sc = Math.Min(fitW, fitH);
            if (sc > MaxScale) sc = MaxScale;
            if (sc < MinScale) sc = MinScale;   // 바닥에 닿으면 이제부터 스크롤
            _scale = sc;

            int needW = (int)(_vW * _scale) + 2 * Pad;
            int needH = (int)(_vH * _scale) + 2 * Pad;
            Size = new Size(Math.Max(viewport.Width, needW), Math.Max(viewport.Height, needH));
            Invalidate();
        }

        public void ClearPlacements()
        {
            foreach (var b in _boxes) { b.Placed = false; b.Rect = RectangleF.Empty; }
            Invalidate();
            RaiseChanged();
        }

        private static EdgeAttachment FindAttachment(IList<EdgeAttachment> list, int channel)
        {
            if (list == null) return null;
            foreach (var a in list) if (a.Channel == channel) return a;
            return null;
        }

        private int IndexOfScreen(string deviceName)
        {
            for (int i = 0; i < _screens.Length; i++)
                if (string.Equals(_screens[i].DeviceName, deviceName, StringComparison.Ordinal)) return i;
            return -1;
        }

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

        private void RaiseChanged()
        {
            var h = AttachmentsChanged;
            if (h != null) h(this, EventArgs.Empty);
        }

        private void RecomputeMonitorRects()
        {
            _monitorPanelRects.Clear();
            if (_screens.Length == 0) return;

            if (_scale <= 0)
            {
                // RelayoutToHost 이전에 그려지는 경우의 폴백 — ClientSize 에 맞춰 스케일 임시 산출.
                ComputeVirtual();
                double f = Math.Min((ClientSize.Width - 2.0 * Pad) / _vW, (ClientSize.Height - 2.0 * Pad) / _vH);
                if (f > MaxScale) f = MaxScale;
                if (f < MinScale) f = MinScale;
                _scale = f;
            }
            // 스케일로 그리고, 콘텐츠(가상*스케일)가 ClientSize 보다 작으면 가운데 정렬.
            double drawnW = _vW * _scale, drawnH = _vH * _scale;
            double offX = Math.Max(Pad, (ClientSize.Width - drawnW) / 2.0);
            double offY = Math.Max(Pad, (ClientSize.Height - drawnH) / 2.0);

            double sumW = 0, sumH = 0;
            foreach (Screen s in _screens)
            {
                Rectangle b = s.Bounds;
                int px = (int)(offX + (b.Left - _vLeft) * _scale);
                int py = (int)(offY + (b.Top - _vTop) * _scale);
                int pw = (int)(b.Width * _scale);
                int ph = (int)(b.Height * _scale);
                _monitorPanelRects.Add(new HandoffRect(px, py, pw, ph));
                sumW += pw; sumH += ph;
            }
            // 채널 박스 = 모니터 평균 크기(같은 스케일).
            _boxW = (float)(sumW / _screens.Length);
            _boxH = (float)(sumH / _screens.Length);
        }

        private RectangleF BoxDisplayRect(ChannelBox box, int trayIndex)
        {
            if (box == _dragging || !box.Placed)
            {
                if (box.Rect.Width > 0) return box.Rect;
                // 미배치(트레이) 박스는 하단 한 줄에 모두 들어오도록 폭을 줄인다 — 개수가 많아도 잘리지 않게.
                int count = 0;
                foreach (var b in _boxes) if (!b.Placed && b != _dragging) count++;
                if (count < 1) count = 1;
                const float gap = 12f;
                float avail = ClientSize.Width - 2f * Pad - (count - 1) * gap;
                float tw = Math.Min(_boxW * 0.78f, avail / count);
                if (tw < 1f) tw = 1f;
                float aspect = _boxW > 1f ? (_boxH / _boxW) : 0.6f;
                float th = tw * aspect;
                float x = Pad + trayIndex * (tw + gap);
                float y = ClientSize.Height - th - Pad - 2;
                return new RectangleF(x, y, tw, th);
            }
            if (box.MonitorIndex < 0 || box.MonitorIndex >= _monitorPanelRects.Count)
                return new RectangleF(Pad, Pad, _boxW, _boxH);
            HandoffRect snapped = HandoffLayout.SnapFlush(_monitorPanelRects[box.MonitorIndex], box.Edge, (int)_boxW, (int)_boxH);
            return new RectangleF(snapped.Left, snapped.Top, snapped.Width, snapped.Height);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            RecomputeMonitorRects();
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Theme.CanvasBg);
            DrawGrid(g);

            // 모니터.
            for (int i = 0; i < _monitorPanelRects.Count; i++)
            {
                HandoffRect m = _monitorPanelRects[i];
                var rect = new RectangleF(m.Left, m.Top, m.Width, m.Height);
                bool primary = i == _primaryIndex;
                using (var fill = new SolidBrush(primary ? Theme.MonPrimaryFill : Theme.MonFill))
                using (var pen = new Pen(primary ? Theme.Accent : Theme.MonStroke, 1.5f))
                using (var path = UiPaint.RoundRect(rect, 5f))
                {
                    g.FillPath(fill, path);
                    g.DrawPath(pen, path);
                }
                DrawCentered(g, rect, "모니터 " + (i + 1), Theme.Ink, FontStyle.Bold, -8);
                DrawCentered(g, rect, primary ? "주 모니터" : "", Theme.Accent, FontStyle.Regular, 10);
            }

            // 채널 박스.
            int tray = 0;
            foreach (var box in _boxes)
            {
                RectangleF r = BoxDisplayRect(box, box.Placed ? 0 : tray);
                if (!box.Placed && box != _dragging) tray++;
                DrawChannelBox(g, box, r);
            }
        }

        private void DrawGrid(Graphics g)
        {
            using (var pen = new Pen(Theme.CanvasGrid, 1f))
            {
                for (int x = 0; x < Width; x += 22) g.DrawLine(pen, x, 0, x, Height);
                for (int y = 0; y < Height; y += 22) g.DrawLine(pen, 0, y, Width, y);
            }
        }

        private void DrawChannelBox(Graphics g, ChannelBox box, RectangleF r)
        {
            bool auto = box.Placed;
            Color fill = auto ? Theme.PcAutoFill : Theme.PcManualFill;
            Color stroke = auto ? Theme.Accent : Theme.PcManualStroke;
            using (var path = UiPaint.RoundRect(r, 5f))
            using (var fb = new SolidBrush(fill))
            {
                g.FillPath(fb, path);
                using (var pen = new Pen(stroke, 1.5f))
                {
                    if (!auto) pen.DashStyle = DashStyle.Dash;
                    g.DrawPath(pen, path);
                }
            }
            float gs = Math.Min(r.Width, r.Height) * 0.34f;
            Icons.Monitor(g, new RectangleF(r.X + (r.Width - gs) / 2, r.Y + (r.Height - gs) / 2, gs, gs),
                auto ? Theme.Accent : Theme.Muted);

            // 라벨: 별칭 + 자동/수동. UiFont.Get 은 캐시 폰트라 dispose 하지 않는다.
            string alias = _aliasFor(box.Channel);
            string line = string.Format("채널 {0} · {1}", box.Channel + 1, alias);
            Font fTitle = UiFont.Get(11f, FontStyle.Bold);
            using (var fb = new SolidBrush(Theme.Ink))
                g.DrawString(line, fTitle, fb, r.X, r.Y - fTitle.Height - 2);
            Font fSub = UiFont.Get(10f);
            using (var fb = new SolidBrush(auto ? Theme.Accent : Theme.Muted))
                g.DrawString(auto ? "접촉 · 자동" : "분리 · 수동", fSub, fb, r.X, r.Bottom + 2);
        }

        private void DrawCentered(Graphics g, RectangleF rect, string text, Color color, FontStyle style, float dy)
        {
            if (string.IsNullOrEmpty(text)) return;
            Font f = UiFont.Get(10.5f, style);   // 캐시 폰트 — dispose 금지
            using (var b = new SolidBrush(color))
            using (var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center })
                g.DrawString(text, f, b, new RectangleF(rect.X, rect.Y + dy, rect.Width, rect.Height), sf);
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (_monitorPanelRects.Count == 0) RecomputeMonitorRects();
            for (int i = _boxes.Count - 1; i >= 0; i--)
            {
                int ti = 0;
                // 트레이 인덱스(미배치 박스의 그리기 순서와 일치).
                for (int j = 0; j < i; j++) if (!_boxes[j].Placed) ti++;
                RectangleF r = BoxDisplayRect(_boxes[i], ti);
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
            _dragging.Rect = new RectangleF(e.X - _dragOffset.X, e.Y - _dragOffset.Y, _boxW, _boxH);
            Invalidate();
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            if (_dragging == null) return;
            if (_monitorPanelRects.Count == 0) RecomputeMonitorRects();

            var boxRect = new HandoffRect((int)_dragging.Rect.X, (int)_dragging.Rect.Y, (int)_boxW, (int)_boxH);
            int monIdx;
            HandoffEdge edge;
            if (HandoffLayout.ResolveAttachment(_monitorPanelRects, boxRect, SnapPx, out monIdx, out edge))
            {
                _dragging.Placed = true;
                _dragging.MonitorIndex = monIdx;
                _dragging.Edge = edge;
                _dragging.Rect = RectangleF.Empty;
            }
            else
            {
                _dragging.Placed = false;
            }
            _dragging = null;
            Invalidate();
            RaiseChanged();
        }
    }
}
