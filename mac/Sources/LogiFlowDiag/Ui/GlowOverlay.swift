import AppKit

/// glow 그림 — 기준은 오른쪽 변(오른쪽 끝이 화면 끝). 다른 변은 그릴 때 좌표를 돌린다.
final class GlowView: NSView {
    var edge: HandoffEdge = .right { didSet { needsDisplay = true } }
    var progress: Double = 0 { didSet { needsDisplay = true } }
    var peak = false { didSet { needsDisplay = true } }

    // 윈도우판 Theme.Accent(라이트 #007AFF · 다크 #0A84FF).
    private var accent: NSColor {
        let dark = effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        return dark ? NSColor(srgbRed: 0x0A / 255.0, green: 0x84 / 255.0, blue: 1, alpha: 1)
                    : NSColor(srgbRed: 0, green: 0x7A / 255.0, blue: 1, alpha: 1)
    }

    override var isOpaque: Bool { false }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.clear.setFill()
        bounds.fill()
        let inten = max(0, min(1, progress))
        if inten <= 0 && !peak { return }

        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.saveGState()
        // 오른쪽 변 기준 좌표계로 바꾼다: W = 띠 두께 방향, L = 변 길이 방향.
        let w = (edge == .left || edge == .right) ? bounds.width : bounds.height
        let l = (edge == .left || edge == .right) ? bounds.height : bounds.width
        switch edge {
        case .right: break
        case .left: ctx.translateBy(x: bounds.width, y: 0); ctx.scaleBy(x: -1, y: 1)
        case .top: ctx.translateBy(x: bounds.width, y: 0); ctx.rotate(by: .pi / 2)       // 기준 x=W → 창 위쪽 끝
        case .bottom: ctx.translateBy(x: 0, y: bounds.height); ctx.rotate(by: -.pi / 2)  // 기준 x=W → 창 아래쪽 끝
        }

        let a = accent
        func blendWhite(_ t: Double) -> NSColor { a.blended(withFraction: CGFloat(t), of: .white) ?? a }

        // 1) 부드러운 glow 그라데이션(안쪽 투명 → 변쪽 진함).
        let glowA = peak ? 235.0 / 255 : inten * 190 / 255
        if glowA > 0, let g = NSGradient(starting: a.withAlphaComponent(0), ending: a.withAlphaComponent(CGFloat(glowA))) {
            g.draw(in: NSRect(x: 0, y: 0, width: w, height: l), angle: 0)
        }
        // 2) tension 코어: 변에 붙은 띠가 progress 로 두꺼워지고 끝에서 흰빛.
        let coreT = peak ? 40.0 : 2 + inten * 32
        let coreA = peak ? 1.0 : inten * 240 / 255
        blendWhite(peak ? 0.75 : inten * inten * 0.4).withAlphaComponent(CGFloat(coreA)).setFill()
        NSRect(x: w - CGFloat(coreT), y: 0, width: CGFloat(coreT), height: l).fill(using: .sourceOver)
        // 3) 가장 밝은 변 선.
        (peak ? NSColor.white : blendWhite(inten * 0.5)).withAlphaComponent(CGFloat(peak ? 1 : inten)).setFill()
        NSRect(x: w - 2.5, y: 0, width: 2.5, height: l).fill(using: .sourceOver)

        ctx.restoreGState()
    }
}

/// 엣지 glow/tension 애니메이션 — 윈도우 `GlowController`(0.14.3) 와 같은 규칙.
/// dwell 동안 80% 까지, 전환 요청 뒤 90% 로 기다리다 setHost 가 실제로 나간 순간(delivered)
/// 흰빛 최고점을 잠깐 보이고 사라진다. 실패·시간 초과면 최고점 없이 흐려진다. 메인 스레드 전용.
final class GlowController {
    private static let band: CGFloat = 90
    private static let dwellCeiling = 0.8
    private static let sendingLevel = 0.9
    private static let sendTimeout: TimeInterval = 1.5
    private static let peakTicks = 3

    private let window: NSWindow
    private let view = GlowView()
    private var timer: Timer?

    private var hasTarget = false
    private var target = 0.0
    private var shown = 0.0
    private var frame = CGRect.zero
    private var edge = HandoffEdge.right
    private var sending = false
    private var sendStart = Date()
    private var peakLeft = 0

    init() {
        window = NSWindow(contentRect: NSRect(x: -10000, y: -10000, width: 1, height: 1),
                          styleMask: .borderless, backing: .buffered, defer: false)
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = false
        window.ignoresMouseEvents = true
        window.level = .screenSaver
        window.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle, .fullScreenAuxiliary]
        window.isReleasedWhenClosed = false
        window.contentView = view
    }

    func update(_ g: EdgeGlow) {
        if g.fired {
            frame = g.frame; edge = g.edge
            sending = true; sendStart = Date(); peakLeft = 0
            hasTarget = true; target = GlowController.sendingLevel
        } else if sending || peakLeft > 0 {
            return // 전환 중엔 남은 입력이 glow 를 흔들지 않게.
        } else if g.active {
            frame = g.frame; edge = g.edge
            target = max(0, min(1, g.progress)) * GlowController.dwellCeiling
            hasTarget = true
        } else {
            hasTarget = false; target = 0
        }
        startTimer()
    }

    /// setHost 가 실제로 나간 순간(아무 스레드에서나). 최고점을 바로 그리고 잠깐 뒤 숨긴다.
    func delivered() {
        guard Thread.isMainThread else { DispatchQueue.main.async { self.delivered() }; return }
        guard sending else { return }
        sending = false
        peakLeft = GlowController.peakTicks
        render(1, peak: true)
        startTimer()
    }

    /// 전환 실패·건너뜀(아무 스레드에서나). 최고점 없이 흐려진다.
    func failed() {
        guard Thread.isMainThread else { DispatchQueue.main.async { self.failed() }; return }
        sending = false; peakLeft = 0
        hasTarget = false; target = 0
        startTimer()
    }

    private func startTimer() {
        guard timer == nil else { return }
        let t = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    private func stopTimer() {
        timer?.invalidate()
        timer = nil
    }

    private func hide() {
        window.orderOut(nil)
        stopTimer()
    }

    private func tick() {
        if peakLeft > 0 {
            peakLeft -= 1
            if peakLeft == 0 {
                shown = 0; hasTarget = false; target = 0
                hide()
            }
            return
        }
        if sending && Date().timeIntervalSince(sendStart) > GlowController.sendTimeout {
            sending = false; hasTarget = false; target = 0
        }
        let goal = hasTarget ? target : 0
        shown += (goal - shown) * (goal > shown ? 0.5 : 0.3)
        let vis = max(0, min(1, shown))
        if vis < 0.015 && !hasTarget {
            hide()
            return
        }
        render(vis, peak: false)
    }

    private func render(_ progress: Double, peak: Bool) {
        let b = GlowController.band
        let r: NSRect
        switch edge {
        case .right: r = NSRect(x: frame.maxX - b, y: frame.minY, width: b, height: frame.height)
        case .left: r = NSRect(x: frame.minX, y: frame.minY, width: b, height: frame.height)
        case .top: r = NSRect(x: frame.minX, y: frame.maxY - b, width: frame.width, height: b)
        case .bottom: r = NSRect(x: frame.minX, y: frame.minY, width: frame.width, height: b)
        }
        if window.frame != r { window.setFrame(r, display: false) }
        view.edge = edge
        view.peak = peak
        view.progress = progress
        if !window.isVisible { window.orderFrontRegardless() }
        view.display()
    }
}
