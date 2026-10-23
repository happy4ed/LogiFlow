import Foundation

/// 화면 변. macOS 좌표(원점 = 주 화면 왼쪽 아래, y 위로) 기준.
enum HandoffEdge: String, Codable, CaseIterable {
    case left, right, top, bottom

    var label: String {
        switch self {
        case .left: return "왼쪽"
        case .right: return "오른쪽"
        case .top: return "위"
        case .bottom: return "아래"
        }
    }
}

/// 순수 엣지-푸시 판정기. 윈도우 `EdgeDetector`(LocalDex 이식) 그대로.
/// 좁은 띠(bandPt) 안에서 dwellMs 유지 + 바깥 방향 누적 밀기(minPush) 가 차면 1회 true.
/// 드래그(버튼 누름)는 트리거하지 않는다. 전환 뒤엔 변에서 rearmDistance 떨어져야 재무장.
final class EdgeDetector {
    static let defaultBandPt: Double = 2
    static let defaultMinPush: Double = 10
    static let rearmDistance: Double = 16

    var dwellMs: Int
    var bandPt = EdgeDetector.defaultBandPt
    var minPush = EdgeDetector.defaultMinPush

    private var touchStartMs: Int64 = -1
    private var pushAccum: Double = 0
    private(set) var armed = true

    init(dwellMs: Int) {
        self.dwellMs = dwellMs
    }

    func progress(_ nowMs: Int64) -> Double {
        guard touchStartMs >= 0, dwellMs > 0 else { return 0 }
        let elapsed = nowMs - touchStartMs
        if elapsed <= 0 { return 0 }
        return elapsed >= Int64(dwellMs) ? 1 : Double(elapsed) / Double(dwellMs)
    }

    func reset(requireLeaveEdge: Bool) {
        touchStartMs = -1
        pushAccum = 0
        armed = !requireLeaveEdge
    }

    /// 변까지 거리(pt). 커서는 화면 안쪽 [minX, maxX) 범위라 오른쪽·위는 max 에서 뺀다.
    static func distance(_ frame: CGRect, _ edge: HandoffEdge, _ p: CGPoint) -> Double {
        switch edge {
        case .left: return Double(p.x - frame.minX)
        case .right: return Double(frame.maxX - 1 - p.x)
        case .top: return Double(frame.maxY - 1 - p.y)
        case .bottom: return Double(p.y - frame.minY)
        }
    }

    /// NSEvent delta(dx 오른쪽 +, dy 아래쪽 +) 중 해당 변 바깥으로 향하는 성분.
    static func outwardPush(_ edge: HandoffEdge, dx: Double, dy: Double) -> Double {
        switch edge {
        case .left: return dx < 0 ? -dx : 0
        case .right: return dx > 0 ? dx : 0
        case .top: return dy < 0 ? -dy : 0
        case .bottom: return dy > 0 ? dy : 0
        }
    }

    func feed(frame: CGRect, edge: HandoffEdge, point: CGPoint, push: Double, nowMs: Int64, buttonDown: Bool) -> Bool {
        let d = EdgeDetector.distance(frame, edge, point)
        if !armed {
            if d >= EdgeDetector.rearmDistance { armed = true }
            return false
        }
        let atBand = d <= bandPt
        if buttonDown || !atBand {
            touchStartMs = -1
            pushAccum = 0
            return false
        }
        if touchStartMs < 0 {
            touchStartMs = nowMs
            pushAccum = 0
            return false
        }
        if push > 0 { pushAccum += push }
        if nowMs - touchStartMs < Int64(dwellMs) { return false }
        if pushAccum < minPush { return false }
        reset(requireLeaveEdge: true)
        return true
    }
}
