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
    /// dwell 이 차는 동안 색이 진해지고 변 쪽 띠가 두꺼워지며(tension) DwellCeiling 까지 오른다.
    /// 최고점은 setHost 가 실제로 나간 순간(Delivered)에 한 번 보이고 곧바로 사라진다.
    /// 전환이 실패하면(Failed·시간 초과) 최고점 없이 흐려진다.
    /// </summary>
    internal sealed class GlowController : IDisposable
    {
        private const int MaxBand = 90;
        // dwell 동안의 상한 · 전환 요청 후 전달을 기다리는 동안의 밝기. 최고점(1.0)은 전달 순간 몫.
        private const double DwellCeiling = 0.8;
        private const double SendingLevel = 0.9;
        // 전달 신호를 이 시간 안에 못 받으면 실패로 보고 흐려진다.
        private const int SendTimeoutMs = 1500;
        // 전달 순간 최고점을 보이는 틱 수(16ms 단위) — 그다음 바로 숨김.
        private const int PeakTicks = 3;

        private readonly GlowOverlayForm _form;
        private readonly Timer _timer;

        private bool _hasTarget;
        private double _targetProgress;
        private HandoffRect _mon;
        private HandoffEdge _edge;
        private double _shown;
        private bool _sending;
        private int _sendStart;
        private int _peakLeft;

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
                // 전환 요청: 밝기를 끌어올린 채 실제 전달(Delivered)을 기다린다.
                _mon = g.Monitor; _edge = g.Edge;
                _sending = true; _sendStart = Environment.TickCount; _peakLeft = 0;
                _hasTarget = true; _targetProgress = SendingLevel;
            }
            else if (_sending || _peakLeft > 0)
            {
                return; // 전환 중엔 남은 raw 입력이 glow 를 흔들지 않게.
            }
            else if (g.Active)
            {
                _mon = g.Monitor; _edge = g.Edge;
                _targetProgress = Math.Max(0, Math.Min(1, g.Progress)) * DwellCeiling;
                _hasTarget = true;
            }
            else
            {
                _hasTarget = false; _targetProgress = 0;
            }
            if (!_timer.Enabled) _timer.Start();
        }

        /// <summary>setHost 가 실제로 나간 순간(아무 스레드에서나). 최고점을 바로 그리고 잠깐 뒤 숨긴다.</summary>
        public void Delivered()
        {
            if (_form.InvokeRequired)
            {
                try { _form.BeginInvoke((Action)Delivered); }
                catch { }
                return;
            }
            if (!_sending) return;
            _sending = false;
            _peakLeft = PeakTicks;
            try { Render(1.0, true); }
            catch { }
            if (!_timer.Enabled) _timer.Start();
        }

        /// <summary>전환 실패·건너뜀(아무 스레드에서나). 최고점 없이 흐려진다.</summary>
        public void Failed()
        {
            if (_form.InvokeRequired)
            {
                try { _form.BeginInvoke((Action)Failed); }
                catch { }
                return;
            }
            _sending = false; _peakLeft = 0;
            _hasTarget = false; _targetProgress = 0;
            if (!_timer.Enabled) _timer.Start();
        }

        private void OnTick(object sender, EventArgs e)
        {
            if (_peakLeft > 0)
            {
                if (--_peakLeft == 0)
                {
                    _shown = 0; _hasTarget = false; _targetProgress = 0;
                    _form.HideOverlay();
                    _timer.Stop();
                }
                return;
            }
            if (_sending && Environment.TickCount - _sendStart > SendTimeoutMs)
            {
                _sending = false; _hasTarget = false; _targetProgress = 0;
            }

            double target = _hasTarget ? _targetProgress : 0;
            // 오를 때는 빠르게 따라가고, 내릴 때는 부드럽게.
            _shown += (target - _shown) * (target > _shown ? 0.5 : 0.3);

            double vis = _shown;
            if (vis < 0) vis = 0; if (vis > 1) vis = 1;

            if (vis < 0.015 && !_hasTarget)
            {
                _form.HideOverlay();
                _timer.Stop();
                return;
            }
            try { Render(vis, false); }
            catch { }
        }

        private void Render(double progress, bool peak)
        {
            HandoffRect m = _mon;
            bool horizontal = _edge == HandoffEdge.Top || _edge == HandoffEdge.Bottom;
            int length = horizontal ? m.Width : m.Height;
            if (length <= 0) return;

            Color accent = Theme.Accent;
            using (Bitmap canon = RenderCanonical(length, progress, peak, accent))
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
        private static Bitmap RenderCanonical(int length, double progress, bool peak, Color accent)
        {
            int W = MaxBand;
            var bmp = new Bitmap(W, Math.Max(1, length), PixelFormat.Format32bppArgb);
            using (var g = Graphics.FromImage(bmp))
            {
                g.SmoothingMode = SmoothingMode.AntiAlias;
                g.Clear(Color.Transparent);

                double inten = progress; if (inten < 0) inten = 0; if (inten > 1) inten = 1;
                // progress 0 이면 아무것도 안 보이고, 쌓일수록 진해진다. peak = 전달 순간의 흰빛 최고점.

                // 1) 부드러운 glow 그라데이션(안쪽 투명 → 변쪽 진함). 색이 진해지는 느낌.
                int glowA = peak ? 235 : (int)(inten * 190);
                if (glowA > 0)
                {
                    using (var br = new LinearGradientBrush(new Rectangle(0, 0, W, bmp.Height),
                        Color.FromArgb(0, accent), Color.FromArgb(glowA, accent), 0f))
                        g.FillRectangle(br, 0, 0, W, bmp.Height);
                }

                // 2) tension 코어: 변에 붙은 띠가 progress 로 두꺼워지고 끝에서 흰빛이 돈다.
                double coreT = peak ? 40 : 2 + inten * 32;
                int coreA = peak ? 255 : (int)(inten * 240);
                Color coreC = Theme.Blend(Color.White, accent, peak ? 0.75 : inten * inten * 0.4);
                using (var cb = new SolidBrush(Color.FromArgb(coreA, coreC)))
                    g.FillRectangle(cb, (float)(W - coreT), 0, (float)coreT, bmp.Height);

                // 3) 가장 밝은 변 선.
                int lineA = peak ? 255 : (int)(inten * 255);
                Color lineC = peak ? Color.White : Theme.Blend(Color.White, accent, inten * 0.5);
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
