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
    /// progress 가 쌓이면 색이 진해지고 변 쪽 띠가 두꺼워지며(tension), dwell 이 차기 직전(PeakAt)
    /// 가장 밝아진다. 전환 순간에는 즉시 사라진다(전환 뒤에 빛이 남거나 새로 켜지지 않게).
    /// </summary>
    internal sealed class GlowController : IDisposable
    {
        private const int MaxBand = 90;
        // dwell 의 이 비율에서 최대 밝기 도달 → 전환 직전까지 최대 유지.
        private const double PeakAt = 0.85;

        private readonly GlowOverlayForm _form;
        private readonly Timer _timer;

        private bool _hasTarget;
        private double _targetProgress;
        private HandoffRect _mon;
        private HandoffEdge _edge;
        private double _shown;

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
                // 전환 순간: 남은 프레임 없이 바로 숨긴다.
                _hasTarget = false; _targetProgress = 0; _shown = 0;
                _timer.Stop();
                _form.HideOverlay();
                return;
            }
            if (g.Active)
            {
                _mon = g.Monitor; _edge = g.Edge;
                double t = g.Progress / PeakAt;
                _targetProgress = t > 1 ? 1 : t;
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
            // 오를 때는 빠르게 따라가 dwell 전에 최대에 닿고, 내릴 때는 부드럽게.
            _shown += (target - _shown) * (target > _shown ? 0.5 : 0.3);

            double vis = _shown;
            if (vis < 0) vis = 0; if (vis > 1) vis = 1;

            if (vis < 0.015 && !_hasTarget)
            {
                _form.HideOverlay();
                _timer.Stop();
                return;
            }
            try { Render(vis); }
            catch { }
        }

        private void Render(double progress)
        {
            HandoffRect m = _mon;
            bool horizontal = _edge == HandoffEdge.Top || _edge == HandoffEdge.Bottom;
            int length = horizontal ? m.Width : m.Height;
            if (length <= 0) return;

            Color accent = Theme.Accent;
            using (Bitmap canon = RenderCanonical(length, progress, accent))
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
        private static Bitmap RenderCanonical(int length, double progress, Color accent)
        {
            int W = MaxBand;
            var bmp = new Bitmap(W, Math.Max(1, length), PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(bmp))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.Clear(Color.Transparent);

                double inten = progress; if (inten < 0) inten = 0; if (inten > 1) inten = 1;
                // progress 0 이면 아무것도 안 보이고, 쌓일수록 진해진다(최대 = 전환 직전).

                // 1) 부드러운 glow 그라데이션(안쪽 투명 → 변쪽 진함). 색이 진해지는 느낌.
                int glowA = (int)(inten * 190);
                if (glowA > 0)
                {
                    using (var br = new LinearGradientBrush(new Rectangle(0, 0, W, bmp.Height),
                        Color.FromArgb(0, accent), Color.FromArgb(glowA, accent), 0f))
                        g.FillRectangle(br, 0, 0, W, bmp.Height);
                }

                // 2) tension 코어: 변에 붙은 띠가 progress 로 두꺼워지고 끝에서 흰빛이 돈다.
                double coreT = 2 + inten * 32;
                int coreA = (int)(inten * 240);
                Color coreC = Theme.Blend(Color.White, accent, inten * inten * 0.45);
                using (var cb = new SolidBrush(Color.FromArgb(coreA, coreC)))
                    g.FillRectangle(cb, (float)(W - coreT), 0, (float)coreT, bmp.Height);

                // 3) 가장 밝은 변 선.
                int lineA = (int)(inten * 255);
                Color lineC = Theme.Blend(Color.White, accent, inten * 0.5);
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
