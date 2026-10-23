import Foundation
import IOKit.hid

/// 통제된 쓰기: 0x1814 func1 setHost 로 호스트(채널)를 전환한다. 윈도우 `HostSwitcher` 대응.
/// 안전 가드 — (a)0x1814 지원 확인된 기기 (b)targetHost 가 0~hostCount-1 범위.
/// 이 타입 밖에서는 어떤 쓰기도 하지 않는다.
enum HostSwitcher {
    private static let writeTimeoutMs = 400

    /// 모든 기기에 setHost 를 기기 사이 대기 없이 먼저 연달아 쓰고, 그다음 확인한다(윈도우 SwitchAllBurst 와 같음).
    /// 첫 쓰기가 성공한 순간 onDelivered 를 부른다 — glow 최고점 = 실제 전달 시점.
    /// 첫 시도는 깨우기 없이 바로 쓰고, 실패한 기기만 ping 으로 깨운 뒤 재시도.
    ///
    /// macOS 는 리시버 뒤 기기들(마우스·키보드)이 같은 IOHIDDevice 를 공유한다. 입력 콜백이 기기당
    /// 하나뿐이라 같은 인터페이스는 한 번만 열고 그 스트림으로 여러 deviceIndex 에 보낸다.
    static func switchAllBurst(_ devices: [ScanResult], targetHost: Int, log: DiagLogger,
                               say: (String) -> Void, attempts: Int, onDelivered: (() -> Void)?) -> Int {
        var pending = [ScanResult]()
        for d in devices {
            let name = d.deviceName ?? "(이름 미상)"
            guard d.device != nil, d.interface != nil else { continue }
            if !d.changeHostSupported || d.changeHostIndex == 0 {
                say("  \(name): 0x1814 미지원 — 건너뜀")
                continue
            }
            if targetHost < 0 || (d.hostCount >= 0 && targetHost >= d.hostCount) {
                say("  \(name): 채널 \(targetHost + 1) 범위 밖(호스트 \(d.hostCount)) — 건너뜀")
                continue
            }
            pending.append(d)
        }

        var switched = 0
        var delivered = false
        var attempt = 1
        while attempt <= attempts && !pending.isEmpty {
            // 인터페이스(path)마다 스트림 하나.
            var streams = [String: (HidDevice, HidppDevice)]()
            defer { for (s, _) in streams.values { s.close() } }
            func hidpp(for d: ScanResult) -> HidppDevice? {
                if let pair = streams[d.path] { return pair.1 }
                guard let dev = d.device, let info = d.interface else { return nil }
                let s = HidDevice(device: dev, info: info)
                guard s.open() else { return nil }
                let h = HidppDevice(stream: s, log: log)
                streams[d.path] = (s, h)
                return h
            }

            // 1) 쓰기 묶음.
            var wrote = [Bool](repeating: false, count: pending.count)
            for (i, d) in pending.enumerated() {
                guard let h = hidpp(for: d) else {
                    say("  \(d.deviceName ?? "?"): 시도\(attempt) 인터페이스 열기 실패")
                    continue
                }
                if attempt > 1 { _ = h.ping(deviceIndex: d.deviceIndex, timeoutMs: 200) }
                wrote[i] = h.setHost(deviceIndex: d.deviceIndex, changeHostIndex: d.changeHostIndex,
                                     targetHost: UInt8(targetHost), writeTimeoutMs: writeTimeoutMs, drainMs: 0)
                if wrote[i] && !delivered {
                    delivered = true
                    onDelivered?()
                }
            }

            // 2) 확인 — 전환되면 기기가 이 리시버에서 사라져 응답이 없는 게 정상(= 성공 신호).
            var next = [ScanResult]()
            for (i, d) in pending.enumerated() {
                var ok = false
                var check = "-"
                if wrote[i], let h = hidpp(for: d) {
                    do {
                        if let info = try h.getHostInfo(deviceIndex: d.deviceIndex, changeHostIndex: d.changeHostIndex, timeoutMs: 300) {
                            ok = info.currHost == targetHost
                            check = "현재채널 \(info.currHost + 1)"
                        } else {
                            ok = true
                            check = "응답없음(전환추정)"
                        }
                    } catch {
                        check = "확인 오류 \(error)"
                    }
                }
                say("  \(d.deviceName ?? "?"): 시도\(attempt) 쓰기=\(wrote[i] ? "성공" : "실패") 확인=\(check) → \(ok ? "전환됨" : "미전환")")
                if ok { switched += 1 } else { next.append(d) }
            }
            pending = next
            if !pending.isEmpty && attempt < attempts { Thread.sleep(forTimeInterval: 0.15) }
            attempt += 1
        }
        return switched
    }
}
