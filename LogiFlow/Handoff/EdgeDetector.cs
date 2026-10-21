using System;
using System.Collections.Generic;

namespace LogiFlow.Handoff
{
    // 출처: LocalDex Shared/InputHandoff.cs 의 InputHandoffEdgeDetector / OutwardPush /
    // InputHandoffLayout(ResolveAttachment·SnapFlush) 를 이식. 좌표계는 논리 픽셀(GetCursorPos 와
    // Screen.Bounds 가 같은 공간 — 프로세스가 per-monitor DPI-aware 가 아니라 일치).

    /// <summary>
    /// 순수 엣지-푸시 판정기. raw 마우스에서 먹이고 절대 블로킹하지 않는다.
    /// 좁은 띠(BandPx) 안에서 DwellMs 유지 + 바깥 방향 누적 푸시(MinPushPx) 가 차면 1회 true.
    /// 드래그(버튼 누름)·주입 입력은 트리거하지 않는다. 핑퐁 방지 재무장(RearmDistance) 포함.
    /// </summary>
    public sealed class EdgeDetector
    {
        public const int DefaultDwellMs = 330;
        public const int DefaultBandPx = 2;
        public const int DefaultMinPushPx = 10;
        public const int RearmDistance = 16;

        private long _touchStartMs = -1;
        private int _pushAccum;
        private bool _armed = true;

        public EdgeDetector()
        {
            DwellMs = DefaultDwellMs;
            BandPx = DefaultBandPx;
            MinPushPx = DefaultMinPushPx;
        }

        public int DwellMs { get; set; }
        public int BandPx { get; set; }
        public int MinPushPx { get; set; }

        public double Progress(long timeMs)
        {
            if (_touchStartMs < 0 || DwellMs <= 0) return 0;
            long elapsed = timeMs - _touchStartMs;
            if (elapsed <= 0) return 0;
            return elapsed >= DwellMs ? 1.0 : elapsed / (double)DwellMs;
        }

        public void Reset(bool requireLeaveEdge)
        {
            _touchStartMs = -1;
            _pushAccum = 0;
            _armed = !requireLeaveEdge;
        }

        public static int DistanceFromEdge(HandoffRect bounds, HandoffEdge edge, int x, int y)
        {
            switch (edge)
            {
                case HandoffEdge.Left: return x - bounds.Left;
                case HandoffEdge.Right: return bounds.Right - 1 - x;
                case HandoffEdge.Top: return y - bounds.Top;
                default: return bounds.Bottom - 1 - y;
            }
        }

        /// <summary>
        /// 포인터가 좁은 띠 안에서 DwellMs 머물며 바깥 밀기를 MinPushPx 만큼 쌓으면 1회 true.
        /// x/y 와 bounds 는 같은(논리) 좌표계여야 한다.
        /// </summary>
        public bool Feed(HandoffRect bounds, HandoffEdge edge, int x, int y,
            int outwardPush, long timeMs, bool injected, bool buttonDown)
        {
            if (injected) return false;
            int distance = DistanceFromEdge(bounds, edge, x, y);
            if (!_armed)
            {
                if (distance >= RearmDistance) _armed = true;
                return false;
            }
            bool atBand = distance <= BandPx;
            if (buttonDown || !atBand)
            {
                _touchStartMs = -1;
                _pushAccum = 0;
                return false;
            }
            if (_touchStartMs < 0)
            {
                _touchStartMs = timeMs;
                _pushAccum = 0;
                return false;
            }
            if (outwardPush > 0) _pushAccum += outwardPush;
            if (timeMs - _touchStartMs < DwellMs) return false;
            if (_pushAccum < MinPushPx) return false;
            Reset(true);
            return true;
        }

        /// <summary>raw 이동(dx,dy) 중 해당 변 바깥으로 향하는 성분.</summary>
        public static int OutwardPush(HandoffEdge edge, int dx, int dy)
        {
            switch (edge)
            {
                case HandoffEdge.Left: return dx < 0 ? -dx : 0;
                case HandoffEdge.Right: return dx > 0 ? dx : 0;
                case HandoffEdge.Top: return dy < 0 ? -dy : 0;
                default: return dy > 0 ? dy : 0;
            }
        }
    }

    /// <summary>캔버스에서 박스를 모니터 변에 스냅/배치하는 레이아웃 계산(LocalDex InputHandoffLayout).</summary>
    public static class HandoffLayout
    {
        public const int DefaultSnapPx = 24;

        private static int Overlap(int a1, int a2, int b1, int b2)
        {
            return Math.Max(0, Math.Min(a2, b2) - Math.Max(a1, b1));
        }

        /// <summary>박스에 가장 가까운 모니터 변을 찾는다. snapPx 안이면 true.</summary>
        public static bool ResolveAttachment(IList<HandoffRect> monitors, HandoffRect box, int snapPx,
            out int monitorIndex, out HandoffEdge edge)
        {
            monitorIndex = -1;
            edge = HandoffEdge.Right;
            if (monitors == null || monitors.Count == 0) return false;
            double best = double.MaxValue;
            for (int i = 0; i < monitors.Count; i++)
            {
                HandoffRect m = monitors[i];
                int vOverlap = Overlap(box.Top, box.Bottom, m.Top, m.Bottom);
                int hOverlap = Overlap(box.Left, box.Right, m.Left, m.Right);
                Consider(vOverlap, Math.Abs(box.Left - m.Right), i, HandoffEdge.Right, ref best, ref monitorIndex, ref edge);
                Consider(vOverlap, Math.Abs(m.Left - box.Right), i, HandoffEdge.Left, ref best, ref monitorIndex, ref edge);
                Consider(hOverlap, Math.Abs(box.Top - m.Bottom), i, HandoffEdge.Bottom, ref best, ref monitorIndex, ref edge);
                Consider(hOverlap, Math.Abs(m.Top - box.Bottom), i, HandoffEdge.Top, ref best, ref monitorIndex, ref edge);
            }
            return monitorIndex >= 0 && best <= snapPx;
        }

        private static void Consider(int overlap, double distance, int index, HandoffEdge candidate,
            ref double best, ref int monitorIndex, ref HandoffEdge edge)
        {
            if (overlap <= 0) return;
            if (distance >= best) return;
            best = distance;
            monitorIndex = index;
            edge = candidate;
        }

        /// <summary>박스를 모니터 변에 밀착(변 따라 가운데 정렬)시킨다.</summary>
        public static HandoffRect SnapFlush(HandoffRect monitor, HandoffEdge edge, int boxWidth, int boxHeight)
        {
            int left;
            int top;
            switch (edge)
            {
                case HandoffEdge.Right:
                    left = monitor.Right;
                    top = monitor.Top + (monitor.Height - boxHeight) / 2;
                    break;
                case HandoffEdge.Left:
                    left = monitor.Left - boxWidth;
                    top = monitor.Top + (monitor.Height - boxHeight) / 2;
                    break;
                case HandoffEdge.Top:
                    left = monitor.Left + (monitor.Width - boxWidth) / 2;
                    top = monitor.Top - boxHeight;
                    break;
                default:
                    left = monitor.Left + (monitor.Width - boxWidth) / 2;
                    top = monitor.Bottom;
                    break;
            }
            return new HandoffRect(left, top, boxWidth, boxHeight);
        }
    }
}
