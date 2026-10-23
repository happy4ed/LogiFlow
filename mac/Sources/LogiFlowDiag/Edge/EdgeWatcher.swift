import AppKit

/// 변 → 채널 배정 1건. 화면은 CGDirectDisplayID 로 기억한다(배치가 바뀌어도 같은 모니터).
struct EdgeBinding: Codable, Hashable {
    var channel: Int          // 0-base
    var displayID: UInt32
    var edge: HandoffEdge
}

/// glow 상태(EdgeWatcher → GlowController). 윈도우 `EdgeGlow` 대응.
struct EdgeGlow {
    var active = false
    var fired = false
    var frame: CGRect = .zero
    var edge: HandoffEdge = .right
    var progress: Double = 0
}

/// 엣지 자동 전환 감시기 — 윈도우 `EdgeWatcher` 의 macOS 판.
/// NSEvent 전역·로컬 모니터로 mouseMoved 의 delta(실제 손 움직임)를 받아, 커서가 화면 끝에 막혀 있어도
/// '바깥으로 미는' 것을 잡는다. 마우스 이벤트 모니터는 손쉬운 사용 권한이 필요 없다(키 이벤트만 필요).
/// 모든 처리는 메인 스레드.
final class EdgeWatcher {
    private final class Attach {
        let binding: EdgeBinding
        let detector: EdgeDetector
        init(_ b: EdgeBinding, dwellMs: Int) {
            binding = b
            detector = EdgeDetector(dwellMs: dwellMs)
        }
    }

    private let log: DiagLogger
    private let onTrigger: (Int) -> Void
    private let onGlow: (EdgeGlow) -> Void
    private var attachments = [Attach]()
    private var globalMonitor: Any?
    private var localMonitor: Any?
    private var lastGlowActive = false
    private var lastSampleMs: Int64 = -1
    private let clock = DispatchTime.now()

    private(set) var running = false

    init(log: DiagLogger, onTrigger: @escaping (Int) -> Void, onGlow: @escaping (EdgeGlow) -> Void) {
        self.log = log
        self.onTrigger = onTrigger
        self.onGlow = onGlow
    }

    var hasAttachments: Bool { !attachments.isEmpty }

    func configure(_ bindings: [EdgeBinding], dwellMs: Int) {
        attachments = bindings.map { Attach($0, dwellMs: dwellMs) }
        log.log("엣지 감시 구성: 배정 \(attachments.count)개, dwell \(dwellMs)ms")
    }

    func start() {
        guard !running else { return }
        globalMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved]) { [weak self] e in
            self?.handle(e)
        }
        localMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved]) { [weak self] e in
            self?.handle(e)
            return e
        }
        running = true
        log.log("엣지 감시 시작")
    }

    func stop() {
        guard running else { return }
        if let m = globalMonitor { NSEvent.removeMonitor(m) }
        if let m = localMonitor { NSEvent.removeMonitor(m) }
        globalMonitor = nil
        localMonitor = nil
        running = false
        if lastGlowActive {
            onGlow(EdgeGlow())
            lastGlowActive = false
        }
        log.log("엣지 감시 중지")
    }

    /// 전환이 실패하면 변을 떠나지 않고 계속 밀어도 다시 시도되게 재무장.
    func rearm() {
        for a in attachments { a.detector.reset(requireLeaveEdge: false) }
    }

    private func nowMs() -> Int64 {
        return Int64((DispatchTime.now().uptimeNanoseconds - clock.uptimeNanoseconds) / 1_000_000)
    }

    static func displayID(_ screen: NSScreen) -> UInt32 {
        let n = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        return n?.uint32Value ?? 0
    }

    private func handle(_ e: NSEvent) {
        guard running, !attachments.isEmpty else { return }
        let p = NSEvent.mouseLocation
        let dx = Double(e.deltaX), dy = Double(e.deltaY)
        let now = nowMs()
        let buttonDown = NSEvent.pressedMouseButtons != 0

        // 커서가 올라가 있는 화면(경계 포함). 오른쪽·위 끝은 maxX-1·maxY-1 근처라 1pt 여유.
        let screen = NSScreen.screens.first { $0.frame.insetBy(dx: -1, dy: -1).contains(p) }
        let curID = screen.map { EdgeWatcher.displayID($0) } ?? 0

        var glowAt: Attach? = nil
        var glowFrame = CGRect.zero
        var glowProgress = 0.0

        for a in attachments {
            guard let screen = screen, a.binding.displayID == curID else {
                a.detector.reset(requireLeaveEdge: false)
                continue
            }
            let frame = screen.frame
            let push = EdgeDetector.outwardPush(a.binding.edge, dx: dx, dy: dy)
            let dist = EdgeDetector.distance(frame, a.binding.edge, p)
            if dist <= 8 && (lastSampleMs < 0 || now - lastSampleMs >= 400) {
                lastSampleMs = now
                log.log(String(format: "엣지 샘플: x=%.1f y=%.1f 변=%@ 채널=%d 거리=%.1f 푸시=%.1f 진행=%.2f",
                               p.x, p.y, a.binding.edge.rawValue, a.binding.channel + 1, dist, push, a.detector.progress(now)))
            }

            if a.detector.feed(frame: frame, edge: a.binding.edge, point: p, push: push, nowMs: now, buttonDown: buttonDown) {
                log.log("엣지 전환 트리거: 변 \(a.binding.edge.rawValue) → 채널 \(a.binding.channel + 1)")
                // 전환 요청: glow 는 실제 전달(GlowController.delivered)을 기다린다.
                onGlow(EdgeGlow(active: true, fired: true, frame: frame, edge: a.binding.edge, progress: 1))
                lastGlowActive = false
                for other in attachments { other.detector.reset(requireLeaveEdge: true) }
                onTrigger(a.binding.channel)
                return
            }

            // 재무장 전엔 변에 남아 있어도 그리지 않는다(넘어간 뒤 파란 변이 남지 않게).
            if !buttonDown && a.detector.armed && dist <= EdgeDetector.defaultBandPt {
                let pr = a.detector.progress(now)
                if glowAt == nil || pr >= glowProgress {
                    glowAt = a
                    glowFrame = frame
                    glowProgress = pr
                }
            }
        }

        if let g = glowAt {
            onGlow(EdgeGlow(active: true, fired: false, frame: glowFrame, edge: g.binding.edge, progress: glowProgress))
            lastGlowActive = true
        } else if lastGlowActive {
            onGlow(EdgeGlow())
            lastGlowActive = false
        }
    }
}
