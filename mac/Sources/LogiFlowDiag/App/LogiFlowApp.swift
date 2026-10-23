import SwiftUI
import AppKit

/// LogiFlow macOS — 로지텍 Easy-Switch 채널 전환·변 밀기 전환·glow (윈도우판 0.14.3 이식).
/// 메뉴 막대에 상주하고, 창은 설정·진단용. 창을 닫아도 앱(변 감시)은 계속 돈다.
@main
struct LogiFlowApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var delegate

    var body: some Scene {
        Settings { EmptyView() }
    }
}

enum AppInfo {
    static var version: String {
        if let v = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String, !v.isEmpty {
            return v
        }
        return "0.2.0"
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let model = AppModel()
    private var window: NSWindow?
    private var statusItem: NSStatusItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        model.start()
        setupStatusItem()
        showWindow()
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        if !flag { showWindow() }
        return true
    }

    @objc func showWindow() {
        if window == nil {
            let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 760, height: 620),
                             styleMask: [.titled, .closable, .miniaturizable, .resizable],
                             backing: .buffered, defer: false)
            w.title = "LogiFlow"
            w.contentViewController = NSHostingController(rootView: ContentView(vm: model))
            w.isReleasedWhenClosed = false
            w.center()
            window = w
        }
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    // MARK: - 메뉴 막대

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.image = NSImage(systemSymbolName: "arrow.left.arrow.right.square", accessibilityDescription: "LogiFlow")
        }
        let menu = NSMenu()
        menu.delegate = self
        item.menu = menu
        statusItem = item
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        menu.removeAllItems()
        let here = model.thisHost
        if model.switchable.isEmpty {
            let none = NSMenuItem(title: "전환 가능한 기기 없음", action: nil, keyEquivalent: "")
            none.isEnabled = false
            menu.addItem(none)
        } else {
            for ch in 0..<model.channelCount {
                let title = ch == here ? "채널 \(ch + 1) (이 Mac)" : "채널 \(ch + 1) 으로 전환"
                let mi = NSMenuItem(title: title, action: ch == here ? nil : #selector(menuSwitch(_:)), keyEquivalent: "")
                mi.target = self
                mi.tag = ch
                mi.isEnabled = ch != here && !model.switching
                menu.addItem(mi)
            }
        }
        menu.addItem(.separator())
        let edge = NSMenuItem(title: "변으로 밀어 넘기기", action: #selector(toggleEdge), keyEquivalent: "")
        edge.target = self
        edge.state = model.edgeEnabled ? .on : .off
        menu.addItem(edge)
        let rescan = NSMenuItem(title: "기기 다시 찾기", action: #selector(rescan), keyEquivalent: "")
        rescan.target = self
        menu.addItem(rescan)
        let open = NSMenuItem(title: "창 열기", action: #selector(showWindow), keyEquivalent: "")
        open.target = self
        menu.addItem(open)
        menu.addItem(.separator())
        menu.addItem(NSMenuItem(title: "종료", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
    }

    @objc private func menuSwitch(_ sender: NSMenuItem) { model.switchTo(channel: sender.tag) }
    @objc private func toggleEdge() { model.edgeEnabled.toggle() }
    @objc private func rescan() { model.refresh() }
}

// MARK: - 창

struct ContentView: View {
    @ObservedObject var vm: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    switchSection
                    edgeSection
                    devicesSection
                }
                .padding(16)
            }
            Divider()
            footer
        }
        .frame(minWidth: 680, minHeight: 520)
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("LogiFlow").font(.headline)
                Text("로지텍 Easy-Switch · v\(AppInfo.version)").font(.caption).foregroundColor(.secondary)
            }
            Spacer()
            if vm.scanning || vm.switching { ProgressView().scaleEffect(0.7).padding(.trailing, 6) }
            Button(action: { vm.refresh() }) {
                Label("기기 다시 찾기", systemImage: "arrow.clockwise")
            }
            .keyboardShortcut("r", modifiers: .command)
            .disabled(vm.scanning)
        }
        .padding(12)
    }

    private var switchSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("채널 전환").font(.headline)
            if vm.switchable.isEmpty {
                Text("전환 가능한 기기 없음 — 마우스·키보드가 이 Mac 에 연결돼 있어야 함")
                    .font(.callout).foregroundColor(.secondary)
            } else {
                HStack(spacing: 8) {
                    ForEach(0..<vm.channelCount, id: \.self) { ch in
                        Button(ch == vm.thisHost ? "채널 \(ch + 1) (이 Mac)" : "채널 \(ch + 1) 으로") {
                            vm.switchTo(channel: ch)
                        }
                        .frame(maxWidth: .infinity)
                        .disabled(ch == vm.thisHost || vm.switching)
                    }
                }
            }
        }
    }

    private var edgeSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("변으로 밀어 넘기기").font(.headline)
                Spacer()
                Toggle("", isOn: $vm.edgeEnabled).labelsHidden().toggleStyle(.switch)
            }
            Text("커서를 화면 끝에 대고 바깥으로 0.5초 밀면 그 채널로 넘어감. 넘어가는 순간 변이 가장 밝게 빛남.")
                .font(.caption).foregroundColor(.secondary)
            ForEach(0..<vm.channelCount, id: \.self) { ch in
                if ch != vm.thisHost {
                    HStack {
                        Text("채널 \(ch + 1)").frame(width: 70, alignment: .leading)
                        Picker("", selection: Binding<String>(
                            get: {
                                guard let b = vm.binding(for: ch) else { return "" }
                                return "\(b.displayID):\(b.edge.rawValue)"
                            },
                            set: { v in
                                let parts = v.split(separator: ":")
                                if parts.count == 2, let id = UInt32(parts[0]), let e = HandoffEdge(rawValue: String(parts[1])) {
                                    vm.setBinding(channel: ch, displayID: id, edge: e)
                                } else {
                                    vm.setBinding(channel: ch, displayID: nil, edge: nil)
                                }
                            })) {
                            Text("쓰지 않음").tag("")
                            ForEach(vm.screens) { s in
                                ForEach(HandoffEdge.allCases, id: \.self) { e in
                                    Text("\(s.name) · \(e.label) 변").tag("\(s.id):\(e.rawValue)")
                                }
                            }
                        }
                        .labelsHidden()
                    }
                }
            }
        }
    }

    private var devicesSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("기기").font(.headline)
            if vm.results.isEmpty {
                Text(vm.scanning ? "찾는 중…" : "응답한 HID++ 기기 없음 — 기기가 연결·깨어 있는지 확인 후 「기기 다시 찾기」")
                    .font(.callout).foregroundColor(.secondary)
            } else {
                ForEach(vm.results) { r in DeviceRow(result: r) }
            }
        }
    }

    private var footer: some View {
        HStack {
            Text(vm.statusLine).font(.caption).foregroundColor(.secondary)
            Spacer()
            Text(vm.logger.logPath.path).font(.caption2).foregroundColor(.secondary).textSelection(.enabled)
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
                field("채널 수", result.hostCount >= 0 ? String(result.hostCount) : "-")
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
