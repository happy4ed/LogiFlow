using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Windows.Forms;
using LogiFlow.Handoff;
using LogiFlow.Native;

namespace LogiFlow.Ui
{
    /// <summary>
    /// 레이어드·클릭 통과·비활성·최상위 오버레이 창. 입력을 전혀 가로채지 않는다.
    /// UpdateLayeredWindow 로 per-pixel alpha glow 를 그린다.
    /// </summary>
    internal sealed class GlowOverlayForm : Form
    {
        private bool _shown;

        public GlowOverlayForm()
        {
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            Bounds = new Rectangle(-10000, -10000, 1, 1);
        }

        protected override bool ShowWithoutActivation { get { return true; } }

        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams cp = base.CreateParams;
                cp.ExStyle |= NativeMethods.WS_EX_LAYERED | NativeMethods.WS_EX_TRANSPARENT
                            | NativeMethods.WS_EX_NOACTIVATE | NativeMethods.WS_EX_TOOLWINDOW
                            | NativeMethods.WS_EX_TOPMOST;
                return cp;
            }
        }

        public void UpdateLayered(Bitmap bmp, Point screenPos)
        {
            if (!IsHandleCreated)
            {
                var force = Handle;
                GC.KeepAlive(force);
            }
            IntPtr screenDc = NativeMethods.GetDC(IntPtr.Zero);
            IntPtr memDc = NativeMethods.CreateCompatibleDC(screenDc);
            IntPtr hBmp = IntPtr.Zero;
            IntPtr old = IntPtr.Zero;
            try
            {
                hBmp = bmp.GetHbitmap(Color.FromArgb(0));
                old = NativeMethods.SelectObject(memDc, hBmp);
                var size = new NativeMethods.SIZE(bmp.Width, bmp.Height);
                var pos = new NativeMethods.POINT { X = screenPos.X, Y = screenPos.Y };
                var src = new NativeMethods.POINT { X = 0, Y = 0 };
                var blend = new NativeMethods.BLENDFUNCTION
                {
                    BlendOp = NativeMethods.AC_SRC_OVER,
                    BlendFlags = 0,
                    SourceConstantAlpha = 255,
                    AlphaFormat = NativeMethods.AC_SRC_ALPHA,
                };
                NativeMethods.UpdateLayeredWindow(Handle, screenDc, ref pos, ref size, memDc, ref src, 0, ref blend, NativeMethods.ULW_ALPHA);
            }
            finally
            {
                if (old != IntPtr.Zero) NativeMethods.SelectObject(memDc, old);
                if (hBmp != IntPtr.Zero) NativeMethods.DeleteObject(hBmp);
                NativeMethods.DeleteDC(memDc);
                NativeMethods.ReleaseDC(IntPtr.Zero, screenDc);
            }
        }

        public void ShowNoActivate()
        {
            if (!_shown)
            {
                NativeMethods.ShowWindow(Handle, NativeMethods.SW_SHOWNOACTIVATE);
                _shown = true;
            }
        }

        public void HideOverlay()
        {
            if (_shown)
            {
                NativeMethods.ShowWindow(Handle, 0); // SW_HIDE
                _shown = false;
            }
        }
    }

    /// <summary>
    /// 엣지 glow/tension 애니메이션. EdgeWatcher 의 progress 를 받아 ~60fps 로 부드럽게 반영한다.
    /// progress 가 쌓이면 색이 진해지고 변 쪽 띠가 두꺼워지며(tension), 전환 순간 pop/flash 후 놓는다.
    /// </summary>
    internal sealed class GlowController : IDisposable
    {
        private const int MaxBand = 90;

        private readonly GlowOverlayForm _form;
        private readonly Timer _timer;

        private bool _hasTarget;
        private double _targetProgress;
        private HandoffRect _mon;
        private HandoffEdge _edge;
        private double _shown;
        private double _pop;
        private bool _popping;

        public GlowController()
        {
            _form = new GlowOverlayForm();
            var force = _form.Handle;
            GC.KeepAlive(force);
            _timer = new Timer { Interval = 16 };
            _timer.Tick += OnTick;
        }

        /// <summary>EdgeWatcher 에서 UI 스레드로 호출.</summary>
        public void Update(EdgeGlow g)
        {
            if (g.Fired)
            {
                _mon = g.Monitor; _edge = g.Edge;
                _popping = true; _pop = 1.0;
                _hasTarget = false; _targetProgress = 0;
            }
            else if (g.Active)
            {
                _mon = g.Monitor; _edge = g.Edge;
                _targetProgress = g.Progress;
                _hasTarget = true;
            }
            else
            {
                _hasTarget = false; _targetProgress = 0;
            }
            if (!_timer.Enabled) _timer.Start();
        }

        private void OnTick(object sender, EventArgs e)
        {
            double target = _hasTarget ? _targetProgress : 0;
            _shown += (target - _shown) * 0.30;
            if (_popping)
            {
                _pop -= 0.07;
                if (_pop <= 0) { _pop = 0; _popping = false; }
            }

            double vis = _shown;
            if (vis < 0) vis = 0; if (vis > 1) vis = 1;
            double flash = _popping ? _pop : 0;

            if (vis < 0.015 && flash <= 0 && !_hasTarget)
            {
                _form.HideOverlay();
                _timer.Stop();
                return;
            }
            try { Render(vis, flash); }
            catch { }
        }

        private void Render(double progress, double flash)
        {
            HandoffRect m = _mon;
            bool horizontal = _edge == HandoffEdge.Top || _edge == HandoffEdge.Bottom;
            int length = horizontal ? m.Width : m.Height;
            if (length <= 0) return;

            Color accent = Theme.Accent;
            using (Bitmap canon = RenderCanonical(length, progress, flash, accent))
            {
                int posX, posY;
                switch (_edge)
                {
                    case HandoffEdge.Left:
                        canon.RotateFlip(RotateFlipType.RotateNoneFlipX);
                        posX = m.Left; posY = m.Top; break;
                    case HandoffEdge.Bottom:
                        canon.RotateFlip(RotateFlipType.Rotate90FlipNone);
                        posX = m.Left; posY = m.Bottom - MaxBand; break;
                    case HandoffEdge.Top:
                        canon.RotateFlip(RotateFlipType.Rotate270FlipNone);
                        posX = m.Left; posY = m.Top; break;
                    default: // Right
                        posX = m.Right - MaxBand; posY = m.Top; break;
                }
                _form.UpdateLayered(canon, new Point(posX, posY));
                _form.ShowNoActivate();
            }
        }

        /// <summary>기준(Right 변) glow 비트맵: 폭 MaxBand × 높이 length, 오른쪽이 변.</summary>
        private static Bitmap RenderCanonical(int length, double progress, double flash, Color accent)
        {
            int W = MaxBand;
            var bmp = new Bitmap(W, Math.Max(1, length), PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(bmp))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.Clear(Color.Transparent);

                double inten = progress; if (inten < 0) inten = 0; if (inten > 1) inten = 1;

                // 1) 부드러운 glow 그라데이션(안쪽 투명 → 변쪽 진함). 색이 진해지는 느낌.
                int glowA = (int)Math.Min(235, inten * 150 + flash * 120);
                if (glowA > 0)
                {
                    using (var br = new LinearGradientBrush(new Rectangle(0, 0, W, bmp.Height),
                        Color.FromArgb(0, accent), Color.FromArgb(glowA, accent), 0f))
                        g.FillRectangle(br, 0, 0, W, bmp.Height);
                }

                // 2) tension 코어: 변에 붙은 띠가 progress 로 두꺼워진다.
                double coreT = 3 + inten * 30 + flash * 26;
                int coreA = (int)Math.Min(255, 150 + inten * 80 + flash * 70);
                Color coreC = flash > 0 ? Theme.Blend(Color.White, accent, Math.Min(0.6, flash * 0.6)) : accent;
                using (var cb = new SolidBrush(Color.FromArgb(coreA, coreC)))
                    g.FillRectangle(cb, (float)(W - coreT), 0, (float)coreT, bmp.Height);

                // 3) 가장 밝은 변 선.
                int lineA = (int)Math.Min(255, 180 + inten * 75 + flash * 80);
                Color lineC = flash > 0 ? Theme.Blend(Color.White, accent, 0.7) : accent;
                using (var lb = new SolidBrush(Color.FromArgb(lineA, lineC)))
                    g.FillRectangle(lb, W - 2.5f, 0, 2.5f, bmp.Height);
            }
            return bmp;
        }

        public void Dispose()
        {
            try { _timer.Stop(); _timer.Dispose(); } catch { }
            try { _form.Dispose(); } catch { }
        }
    }
}
