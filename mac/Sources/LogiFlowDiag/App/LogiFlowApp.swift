import SwiftUI

/// LogiFlow macOS 진단 도구 — 윈도우 버전의 「읽기부터 확인」 첫 단계.
/// 로지텍 HID++ 기기를 찾아 0x1814 정보(기능 인덱스·호스트 개수·현재 채널)를 읽는다.
/// 전환·엣지·풀 UI 는 다음 단계. 쓰기(setHost)는 하지 않는 읽기 전용.
@main
struct LogiFlowApp: App {
    var body: some Scene {
        WindowGroup("LogiFlow 진단 (mac)") {
            ContentView()
                .frame(minWidth: 720, minHeight: 440)
        }
    }
}

enum AppInfo {
    static var version: String {
        if let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String, !v.isEmpty {
            return v
        }
        return "0.1.0"
    }
}

/// 스캔 상태를 담는 뷰 모델. 스캔은 백그라운드에서 돌고 결과만 메인에 반영한다.
final class DiagViewModel: ObservableObject {
    @Published var results: [ScanResult] = []
    @Published var scanning = false
    @Published var statusLine = "준비됨 — 「새로 고침」으로 스캔"
    @Published var logPath = ""

    private let logger = DiagLogger()

    init() {
        logPath = logger.logPath.path
    }

    func refresh() {
        guard !scanning else { return }
        scanning = true
        statusLine = "스캔 중…"
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self = self else { return }
            let scanner = DeviceScanner(log: self.logger)
            let found = scanner.scan()
            DispatchQueue.main.async {
                self.results = found
                self.scanning = false
                let switchable = found.filter { $0.changeHostSupported }.count
                self.statusLine = String(format: "장치 %d개 (전환 가능 0x1814 %d개) · 로그: %@",
                                         found.count, switchable, self.logger.logPath.lastPathComponent)
            }
        }
    }
}

struct ContentView: View {
    @StateObject private var vm = DiagViewModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()
            if vm.results.isEmpty {
                emptyState
            } else {
                deviceTable
            }
            Divider()
            footer
        }
        .onAppear { vm.refresh() }   // 창이 뜨면 자동으로 1회 스캔.
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("LogiFlow 진단").font(.headline)
                Text("로지텍 HID++ · 0x1814 읽기 전용 · v\(AppInfo.version)")
                    .font(.caption).foregroundColor(.secondary)
            }
            Spacer()
            if vm.scanning { ProgressView().scaleEffect(0.7).padding(.trailing, 6) }
            Button(action: { vm.refresh() }) {
                Label("새로 고침", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r", modifiers: .command)
            .disabled(vm.scanning)
        }
        .padding(12)
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Spacer()
            Image(systemName: "dot.radiowaves.left.and.right")
                .font(.system(size: 36)).foregroundColor(.secondary)
            Text(vm.scanning ? "스캔 중…" : "응답한 HID++ 장치 없음")
                .foregroundColor(.secondary)
            if !vm.scanning {
                Text("기기가 연결/깨어 있는지 확인 후 「새로 고침」. 다른 앱(Logi Options+)이\n기기를 선점하면 조회가 막힐 수 있습니다.")
                    .font(.caption).foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var deviceTable: some View {
        ScrollView {
            VStack(spacing: 10) {
                ForEach(vm.results) { r in
                    DeviceRow(result: r)
                }
            }
            .padding(12)
        }
    }

    private var footer: some View {
        HStack {
            Text(vm.statusLine).font(.caption).foregroundColor(.secondary)
            Spacer()
            Text(vm.logPath).font(.caption2).foregroundColor(.secondary).textSelection(.enabled)
        }
        .padding(10)
    }
}

struct DeviceRow: View {
    let result: ScanResult

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(result.deviceName ?? "(이름 미상)").font(.headline)
                Spacer()
                if result.changeHostSupported {
                    badge("전환 가능", color: .green)
                } else {
                    badge("0x1814 미지원", color: .secondary)
                }
            }
            HStack(spacing: 16) {
                field("VID:PID", result.vidPid)
                field("연결", result.connectionType ?? "-")
                field("deviceIndex", String(format: "0x%02X", result.deviceIndex))
            }
            HStack(spacing: 16) {
                field("0x1814 인덱스", result.changeHostIndex != 0 ? String(format: "0x%02X", result.changeHostIndex) : "-")
                field("호스트 개수", result.hostCount >= 0 ? String(result.hostCount) : "-")
                field("현재 채널", result.currentHost >= 0 ? String(result.currentHost + 1) : "-")
            }
            if let note = result.note, !note.isEmpty {
                Text(note).font(.caption).foregroundColor(.orange)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.gray.opacity(0.08)))
    }

    private func field(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(label).font(.caption2).foregroundColor(.secondary)
            Text(value).font(.system(.body, design: .monospaced))
        }
    }

    private func badge(_ text: String, color: Color) -> some View {
        Text(text)
            .font(.caption2).bold()
            .padding(.horizontal, 8).padding(.vertical, 3)
            .background(Capsule().fill(color.opacity(0.18)))
            .foregroundColor(color == .secondary ? .secondary : color)
    }
}
