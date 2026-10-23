using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.Windows.Forms;
using LogiFlow.Config;
using LogiFlow.Diagnostics;
using LogiFlow.Native;

namespace LogiFlow.Handoff
{
    /// <summary>엣지 glow 오버레이 상태(EdgeWatcher → GlowController).</summary>
    internal struct EdgeGlow
    {
        public bool Active;       // 변에 닿아 progress 가 쌓이는 중
        public bool Fired;        // 전환 순간(pop)
        public HandoffRect Monitor;
        public HandoffEdge Edge;
        public double Progress;   // 0..1
        public int Channel;       // 0-base
    }

    /// <summary>
    /// 엣지 자동 전환 감시기. 숨은 메시지 창으로 WM_INPUT(raw 마우스)을 받아,
    /// 커서가 설정된 모니터 변의 좁은 띠에서 dwell+바깥밀기를 만족하면 그 변의 채널로 전환을 트리거한다.
    /// 좌표계: GetCursorPos + Screen.Bounds 모두 논리 픽셀(LocalDex 가 검증한 방식).
    /// 전환 후엔 이 PC 에서 기기가 사라지므로 이 PC 는 더 명령을 보낼 수 없다(복귀는 대상 PC/기기 버튼).
    /// </summary>
    internal sealed class EdgeWatcher : IDisposable
    {
        private sealed class Attach
        {
            public int Channel;
            public string MonitorDeviceName;
            public HandoffEdge Edge;
            public HandoffRect Bounds;
            public readonly EdgeDetector Detector = new EdgeDetector();
        }

        private sealed class MessageWindow : NativeWindow
        {
            private readonly EdgeWatcher _owner;
            public MessageWindow(EdgeWatcher owner)
            {
                _owner = owner;
                var cp = new CreateParams();
                cp.Caption = "LogiFlowEdgeWatcher";
                cp.Parent = new IntPtr(-3); // HWND_MESSAGE: 메시지 전용 창
                CreateHandle(cp);
            }

            protected override void WndProc(ref Message m)
            {
                if (m.Msg == NativeMethods.WM_INPUT)
                {
                    _owner.OnRawInput(m.LParam);
                }
                base.WndProc(ref m);
            }
        }

        private readonly DiagLogger _log;
        private readonly Action<int> _onTrigger;
        private readonly Stopwatch _clock = Stopwatch.StartNew();
        private readonly List<Attach> _attachments = new List<Attach>();
        private MessageWindow _window;
        private bool _running;
        private int _dwellMs = EdgeDetector.DefaultDwellMs;
        private long _lastSampleMs = -1;
        private readonly Action<EdgeGlow> _onGlow;
        private bool _lastGlowActive;

        internal EdgeWatcher(DiagLogger log, Action<int> onTrigger, Action<EdgeGlow> onGlow)
        {
            _log = log;
            _onTrigger = onTrigger;
            _onGlow = onGlow;
        }

        public bool IsRunning { get { return _running; } }

        /// <summary>설정의 배치를 현재 모니터 배치에 맞춰 다시 구성한다.</summary>
        public void Configure(AppConfig config)
        {
            _dwellMs = config.DwellMs;
            _attachments.Clear();
            foreach (var a in config.Attachments)
            {
                HandoffRect bounds;
                if (!TryMonitorBounds(a.MonitorDeviceName, out bounds))
                {
                    _log.Log("엣지 설정 건너뜀(모니터 없음): " + a.MonitorDeviceName);
                    continue;
                }
                var at = new Attach
                {
                    Channel = a.Channel,
                    MonitorDeviceName = a.MonitorDeviceName,
                    Edge = a.Edge,
                    Bounds = bounds,
                };
                at.Detector.DwellMs = _dwellMs;
                _attachments.Add(at);
            }
            _log.Log(string.Format("엣지 감시 구성: 배치 {0}개, dwell {1}ms", _attachments.Count, _dwellMs));
        }

        public bool HasAttachments { get { return _attachments.Count > 0; } }

        public void Start()
        {
            if (_running) return;
            if (_window == null)
            {
                _window = new MessageWindow(this);
            }
            if (!NativeMethods.RegisterMouseRawInput(_window.Handle, true))
            {
                _log.LogError("raw input 등록 실패 — 엣지 감시 비활성");
                return;
            }
            _running = true;
            _log.Log("엣지 감시 시작");
        }

        public void Stop()
        {
            if (!_running) return;
            try
            {
                if (_window != null)
                {
                    NativeMethods.RegisterMouseRawInput(_window.Handle, false);
                }
            }
            catch
            {
            }
            _running = false;
            if (_lastGlowActive)
            {
                ReportGlow(new EdgeGlow { Active = false });
                _lastGlowActive = false;
            }
            _log.Log("엣지 감시 중지");
        }

        private void OnRawInput(IntPtr lParam)
        {
            if (!_running || _attachments.Count == 0) return;

            int dx, dy;
            if (!NativeMethods.TryReadRelativeMouse(lParam, out dx, out dy)) return;

            NativeMethods.POINT cursor;
            if (!NativeMethods.GetCursorPos(out cursor)) return;

            long now = _clock.ElapsedMilliseconds;
            bool buttonDown = Control.MouseButtons != MouseButtons.None;

            // 커서가 올라가 있는 모니터를 찾아, 그 모니터의 변 배치만 먹인다.
            Screen screen = Screen.FromPoint(new Point(cursor.X, cursor.Y));
            string devName = screen != null ? screen.DeviceName : null;

            Attach glowAt = null;
            double glowProgress = 0;

            foreach (var at in _attachments)
            {
                if (!string.Equals(at.MonitorDeviceName, devName, StringComparison.Ordinal))
                {
                    // 다른 모니터에 있으면 그 변의 dwell 은 리셋.
                    at.Detector.Reset(false);
                    continue;
                }

                int push = EdgeDetector.OutwardPush(at.Edge, dx, dy);
                MaybeSample(at, cursor, push, now);

                bool fired = at.Detector.Feed(at.Bounds, at.Edge, cursor.X, cursor.Y,
                    push, now, false, buttonDown);
                if (fired)
                {
                    _log.Log(string.Format("엣지 전환 트리거: 모니터 {0} 변 {1} → 채널 {2}",
                        at.MonitorDeviceName, at.Edge, at.Channel + 1));
                    int channel = at.Channel;
                    // 전환 순간: glow 즉시 숨김.
                    ReportGlow(new EdgeGlow { Active = true, Fired = true, Monitor = at.Bounds, Edge = at.Edge, Progress = 1.0, Channel = at.Channel });
                    _lastGlowActive = false;
                    // 다른 변의 dwell 도 리셋(중복 트리거 방지).
                    foreach (var other in _attachments) other.Detector.Reset(true);
                    if (_onTrigger != null) _onTrigger(channel);
                    return;
                }

                // glow: 변의 좁은 띠에 닿아(드래그 아님) progress 가 쌓이는 중인 변을 고른다.
                // 전환 직후(재무장 전)엔 커서가 변에 남아 있어도 그리지 않는다 — 넘어간 뒤 이 PC 엔
                // 입력이 더 안 와서, 여기서 켜면 끌 기회 없이 파란 변이 남는다.
                int dist = EdgeDetector.DistanceFromEdge(at.Bounds, at.Edge, cursor.X, cursor.Y);
                if (!buttonDown && at.Detector.Armed && dist <= EdgeDetector.DefaultBandPx)
                {
                    double p = at.Detector.Progress(now);
                    if (glowAt == null || p >= glowProgress)
                    {
                        glowAt = at;
                        glowProgress = p;
                    }
                }
            }

            if (glowAt != null)
            {
                ReportGlow(new EdgeGlow { Active = true, Fired = false, Monitor = glowAt.Bounds, Edge = glowAt.Edge, Progress = glowProgress, Channel = glowAt.Channel });
                _lastGlowActive = true;
            }
            else if (_lastGlowActive)
            {
                ReportGlow(new EdgeGlow { Active = false });
                _lastGlowActive = false;
            }
        }

        private void ReportGlow(EdgeGlow g)
        {
            if (_onGlow != null) _onGlow(g);
        }

        private void MaybeSample(Attach at, NativeMethods.POINT cursor, int push, long now)
        {
            int dist = EdgeDetector.DistanceFromEdge(at.Bounds, at.Edge, cursor.X, cursor.Y);
            if (dist > 8) return;
            if (_lastSampleMs >= 0 && now - _lastSampleMs < 400) return;
            _lastSampleMs = now;
            _log.Log(string.Format("엣지 샘플: x={0} y={1} 변={2} 채널={3} 거리={4} 푸시={5} 진행={6:0.00}",
                cursor.X, cursor.Y, at.Edge, at.Channel + 1, dist, push, at.Detector.Progress(now)));
        }

        private static bool TryMonitorBounds(string deviceName, out HandoffRect bounds)
        {
            bounds = new HandoffRect(0, 0, 1, 1);
            foreach (Screen s in Screen.AllScreens)
            {
                if (string.Equals(s.DeviceName, deviceName, StringComparison.Ordinal))
                {
                    Rectangle b = s.Bounds;
                    bounds = new HandoffRect(b.Left, b.Top, b.Width, b.Height);
                    return true;
                }
            }
            return false;
        }

        public void Dispose()
        {
            Stop();
            try
            {
                if (_window != null)
                {
                    _window.DestroyHandle();
                    _window = null;
                }
            }
            catch
            {
            }
        }
    }
}
