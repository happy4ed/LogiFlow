import Foundation
import IOKit.hid

/// 로지텍 HID++ 장치를 열거하고, 각 후보에 대해 런타임으로
/// 0x1814 인덱스·호스트 정보·기기 이름을 조회한다. 읽기 전용.
/// 윈도우 `DeviceScanner` 의 로직을 그대로 옮긴 것(전송만 IOKit).
final class DeviceScanner {
    static let logitechVid: UInt16 = 0x046D
    static let hidppUsagePage: UInt16 = 0xFF43
    static let usageLong: UInt16 = 0x0202
    static let usageShort: UInt16 = 0x0201

    // deviceIndex 후보: 직접 연결(0xFF) + 리시버 뒤 1~6.
    private static let deviceIndexCandidates: [UInt8] = [0xFF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06]

    private static let probeTimeoutMs = 600
    private static let queryTimeoutMs = 800

    private let log: DiagLogger

    init(log: DiagLogger) {
        self.log = log
    }

    /// 연결된 로지텍 HID 인터페이스 식별자의 지문(변화 감지용·HID++ I/O 없음).
    static func signature() -> String {
        let paths = HidManager.enumerate(vendorIdFilter: logitechVid, usagePageFilter: 0)
            .map { $0.path }
            .sorted()
        return paths.joined(separator: "|")
    }

    func scan() -> [ScanResult] {
        var results = [ScanResult]()

        log.log("=== 스캔 시작 ===")
        // usagePage 필터 없이 VID 046D 인터페이스를 전부 가져와 진단에 남긴다.
        let devices = HidManager.openDevices(vendorIdFilter: DeviceScanner.logitechVid)

        log.log(String(format: "로지텍(VID 046D) HID 인터페이스 %d개 발견", devices.count))
        for (iface, _) in devices {
            log.log(String(format: "  iface %@ usagePage=0x%04X usage=0x%04X in=%d out=%d caps=%@ path=%@",
                           iface.vidPid, iface.usagePage, iface.usage, iface.inputReportByteLength,
                           iface.outputReportByteLength, iface.capsRead ? "ok" : "unread", iface.path))
        }

        for (iface, device) in devices {
            // HID++ 후보: long(usage 0x0202, 20바이트) + short(usage 0x0201, 7바이트)
            // 벤더 컬렉션, 그리고 벤더 usagePage(0xFFxx)에 7바이트 이상 출력 리포트가 있는 인터페이스.
            let candidate = iface.capsRead &&
                (iface.usage == DeviceScanner.usageLong || iface.usage == DeviceScanner.usageShort ||
                 (iface.usagePage >= 0xFF00 && iface.outputReportByteLength >= 7))
            log.log(String(format: "  iface %@ usage=0x%04X 후보=%@",
                           iface.vidPid, iface.usage, candidate ? "예" : "아니오"))
            if !candidate {
                continue
            }

            probeInterface(iface: iface, device: device, results: &results)
        }

        let deduped = DeviceScanner.deduplicate(results)

        if deduped.isEmpty {
            log.log("응답한 HID++ 장치 없음")
        }
        log.log(String(format: "=== 스캔 종료: 후보 응답 %d건 → 중복 제거 후 장치 %d개 ===",
                       results.count, deduped.count))
        return deduped
    }

    /// 같은 물리 기기가 여러 인터페이스(long/short)로 잡히면 하나로 합친다.
    /// 키 = 이름 + deviceIndex + 연결 방식. 정보가 더 풍부한 쪽(0x1814 지원)을 유지한다.
    private static func deduplicate(_ results: [ScanResult]) -> [ScanResult] {
        var map = [String: ScanResult]()
        var order = [String]()
        for r in results {
            let key = (r.deviceName ?? "") + "|" + String(r.deviceIndex) + "|" + (r.connectionType ?? "")
            if let existing = map[key] {
                if !existing.changeHostSupported && r.changeHostSupported {
                    map[key] = r  // 0x1814 정보를 더 많이 가진 쪽으로 교체.
                }
            } else {
                map[key] = r
                order.append(key)
            }
        }
        return order.compactMap { map[$0] }
    }

    private func probeInterface(iface: HidInterfaceInfo, device: IOHIDDevice, results: inout [ScanResult]) {
        let stream = HidDevice(device: device, info: iface)
        if !stream.open() {
            log.logError(String(format: "인터페이스 열기 실패 %@", iface.path))
            return
        }
        defer { stream.close() }

        let hidpp = HidppDevice(stream: stream, log: log)

        for deviceIndex in DeviceScanner.deviceIndexCandidates {
            // 잠자는 기기(특히 키보드)를 고려해 1회 재시도.
            let present = hidpp.ping(deviceIndex: deviceIndex, timeoutMs: DeviceScanner.probeTimeoutMs)
                || hidpp.ping(deviceIndex: deviceIndex, timeoutMs: DeviceScanner.probeTimeoutMs)

            // 각 인터페이스×deviceIndex 핑 결과를 한 줄씩 남긴다.
            log.log(String(format: "  ping %@ dev=0x%02X → %@",
                           iface.vidPid, deviceIndex, present ? "응답" : "타임아웃"))

            if !present {
                continue
            }

            results.append(buildResult(iface: iface, hidpp: hidpp, deviceIndex: deviceIndex))
        }
    }

    private func buildResult(iface: HidInterfaceInfo, hidpp: HidppDevice, deviceIndex: UInt8) -> ScanResult {
        let r = ScanResult()
        r.vidPid = iface.vidPid
        r.path = iface.path
        r.deviceIndex = deviceIndex
        r.connectionType = deviceIndex == 0xFF
            ? "직접(USB/BT)"
            : String(format: "리시버 뒤(index %d)", deviceIndex)

        // 0x1814 Change Host 인덱스 런타임 조회.
        do {
            let chIndex = try hidpp.getFeatureIndex(deviceIndex: deviceIndex,
                                                    featureId: HidppDevice.featureChangeHost,
                                                    timeoutMs: DeviceScanner.queryTimeoutMs)
            if chIndex != 0 {
                r.changeHostSupported = true
                r.changeHostIndex = chIndex
                if let info = try hidpp.getHostInfo(deviceIndex: deviceIndex, changeHostIndex: chIndex,
                                                    timeoutMs: DeviceScanner.queryTimeoutMs) {
                    r.hostCount = info.nbHost
                    r.currentHost = info.currHost
                } else {
                    r.note = DeviceScanner.appendNote(r.note, "getHostInfo 무응답")
                }
            } else {
                r.changeHostSupported = false
                r.note = DeviceScanner.appendNote(r.note, "0x1814 미지원")
            }
        } catch {
            r.note = DeviceScanner.appendNote(r.note, "0x1814 조회 오류: \(error)")
        }

        // 기기 이름: HID++ 0x0005 우선, 실패 시 제품 문자열.
        do {
            let nameIndex = try hidpp.getFeatureIndex(deviceIndex: deviceIndex,
                                                      featureId: HidppDevice.featureDeviceName,
                                                      timeoutMs: DeviceScanner.queryTimeoutMs)
            if nameIndex != 0 {
                r.deviceName = hidpp.getDeviceName(deviceIndex: deviceIndex, nameFeatureIndex: nameIndex,
                                                   timeoutMs: DeviceScanner.queryTimeoutMs)
            }
        } catch {
            log.logError(String(format: "0x0005 조회 오류 dev=0x%02X: %@", deviceIndex, "\(error)"))
        }

        if (r.deviceName ?? "").isEmpty {
            r.deviceName = !(iface.productString ?? "").isEmpty ? iface.productString : "(이름 미상)"
        }

        return r
    }

    private static func appendNote(_ existing: String?, _ add: String) -> String {
        guard let existing = existing, !existing.isEmpty else { return add }
        return existing + "; " + add
    }
}
