import AppKit
import Combine

/// 화면 1개(설정 피커용).
struct ScreenChoice: Identifiable, Hashable {
    let id: UInt32       // CGDirectDisplayID
    let name: String
}

/// 앱 상태 — 기기 스캔·전환·엣지 감시·glow 를 묶는다. 메인 스레드 전용, HID I/O 는 hidQueue(직렬).
/// 스캔과 전환이 같은 큐라 리시버에서 HID++ 요청이 겹치지 않는다(윈도우 0.14.3 의 스캔 경합 교훈).
final class AppModel: ObservableObject {
    static let dwellMs = 500

    @Published var results: [ScanResult] = []
    @Published var scanning = false
    @Published var switching = false
    @Published var statusLine = "준비됨"
    @Published var screens: [ScreenChoice] = []
    @Published var edgeEnabled: Bool {
        didSet { UserDefaults.standard.set(edgeEnabled, forKey: "edgeEnabled"); applyEdge() }
    }
    @Published var bindings: [EdgeBinding] {
        didSet { saveBindings(); applyEdge() }
    }

    let logger = DiagLogger()
    private let hidQueue = DispatchQueue(label: "LogiFlow.hid", qos: .userInitiated)
    private var glow: GlowController!
    private var watcher: EdgeWatcher!
    private var retryTimer: Timer?

    init() {
        edgeEnabled = UserDefaults.standard.object(forKey: "edgeEnabled") as? Bool ?? true
        if let data = UserDefaults.standard.data(forKey: "edgeBindings"),
           let list = try? JSONDecoder().decode([EdgeBinding].self, from: data) {
            bindings = list
        } else {
            bindings = []
        }
    }

    func start() {
        glow = GlowController()
        watcher = EdgeWatcher(log: logger,
                              onTrigger: { [weak self] ch in self?.switchTo(channel: ch, fromEdge: true) },
                              onGlow: { [weak self] g in self?.glow.update(g) })
        reloadScreens()
        NotificationCenter.default.addObserver(forName: NSApplication.didChangeScreenParametersNotification,
                                               object: nil, queue: .main) { [weak self] _ in
            self?.reloadScreens()
            self?.applyEdge()
        }
        refresh()
        // 기기가 하나도 없으면(잠듦·다른 PC 에 가 있음) 3초마다 조용히 다시 찾는다.
        retryTimer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            guard let self = self else { return }
            if self.switchable.isEmpty && !self.scanning && !self.switching { self.refresh(quiet: true) }
        }
    }

    // MARK: - 기기

    var switchable: [ScanResult] { results.filter { $0.changeHostSupported && $0.hostCount > 0 } }
    var channelCount: Int { max(3, switchable.map { $0.hostCount }.max() ?? 3) }
    /// 이 Mac 의 채널(0-base) — 지금 이 Mac 에 붙은 기기의 현재 채널.
    var thisHost: Int { switchable.first(where: { $0.currentHost >= 0 })?.currentHost ?? -1 }

    func refresh(quiet: Bool = false) {
        guard !scanning else { return }
        scanning = true
        if !quiet { statusLine = "기기 찾는 중…" }
        hidQueue.async { [weak self] in
            guard let self = self else { return }
            let found = DeviceScanner(log: self.logger).scan()
            DispatchQueue.main.async {
                self.results = found
                self.scanning = false
                let n = self.switchable.count
                if !quiet || n > 0 {
                    self.statusLine = n > 0
                        ? "전환 가능 기기 \(n)개 · 이 Mac = 채널 \(self.thisHost >= 0 ? String(self.thisHost + 1) : "?")"
                        : "전환 가능한 기기 없음 — 마우스·키보드가 이 Mac 에 연결돼 있는지"
                }
                self.applyEdge()
            }
        }
    }

    /// 채널 전환(버튼·메뉴·변 밀기 공통). 쓰기는 HostSwitcher 한 경로.
    func switchTo(channel: Int, fromEdge: Bool = false) {
        let devs = switchable
        if switching || devs.isEmpty {
            logger.log(devs.isEmpty ? "전환 건너뜀: 기기 0개" : "전환 건너뜀: 이전 전환 진행 중")
            if fromEdge { glow.failed() }
            return
        }
        switching = true
        statusLine = "채널 \(channel + 1) 으로 전환 중…"
        let log = logger
        let glow: GlowController = self.glow
        let onDelivered: (() -> Void)? = fromEdge ? { glow.delivered() } : nil
        hidQueue.async { [weak self] in
            log.log("[전환] → 채널 \(channel + 1) · 기기 \(devs.count)개\(fromEdge ? " (변 밀기)" : "")")
            let ok = HostSwitcher.switchAllBurst(devs, targetHost: channel, log: log, say: { log.log($0) },
                                                 attempts: 3, onDelivered: onDelivered)
            log.log("  setHost 전환 \(ok)/\(devs.count)개")
            DispatchQueue.main.async {
                guard let self = self else { return }
                self.switching = false
                self.statusLine = "채널 \(channel + 1): \(ok)/\(devs.count)개 전환"
                if ok == 0 {
                    // 하나도 못 넘겼으면 glow 를 흐리고, 계속 밀면 다시 시도되게 재무장 + 기기 다시 찾기.
                    if fromEdge { self.glow.failed(); self.watcher.rearm() }
                    self.refresh(quiet: true)
                }
            }
        }
    }

    // MARK: - 가장자리

    func reloadScreens() {
        screens = NSScreen.screens.enumerated().map { (i, s) in
            ScreenChoice(id: EdgeWatcher.displayID(s), name: "\(i + 1). \(s.localizedName)")
        }
    }

    func binding(for channel: Int) -> EdgeBinding? {
        bindings.first { $0.channel == channel }
    }

    func setBinding(channel: Int, displayID: UInt32?, edge: HandoffEdge?) {
        var list = bindings.filter { $0.channel != channel }
        if let id = displayID, let e = edge {
            // 같은 화면·변을 다른 채널이 쓰고 있으면 넘겨받는다.
            list.removeAll { $0.displayID == id && $0.edge == e }
            list.append(EdgeBinding(channel: channel, displayID: id, edge: e))
        }
        bindings = list.sorted { $0.channel < $1.channel }
    }

    private func saveBindings() {
        if let data = try? JSONEncoder().encode(bindings) {
            UserDefaults.standard.set(data, forKey: "edgeBindings")
        }
    }

    private func applyEdge() {
        guard let watcher = watcher else { return }
        let live = bindings.filter { b in screens.contains { $0.id == b.displayID } && b.channel != thisHost }
        watcher.stop()
        watcher.configure(live, dwellMs: AppModel.dwellMs)
        if edgeEnabled && !live.isEmpty && !switchable.isEmpty {
            watcher.start()
        }
    }

    var edgeRunning: Bool { watcher?.running ?? false }
}
