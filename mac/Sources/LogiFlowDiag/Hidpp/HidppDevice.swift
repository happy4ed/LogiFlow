import Foundation

/// 한 HID++ 인터페이스(long 0x0202 또는 short 0x0201) 위에서 특정 deviceIndex 를 상대로
/// 호출을 수행한다. 출력 리포트 길이로 long(0x11)/short(0x10) 프레임을 자동 선택한다.
/// 조회: func0 / IRoot(0x0000) / DeviceName(0x0005). **쓰기(setHost 0x1814 func1)는 구현하지
/// 않는다** — 이 macOS 판은 읽기 전용 진단 도구다.
///
/// 윈도우 `LogiFlow/Hidpp/HidppDevice.cs` 의 프로토콜 로직을 그대로 옮긴 것이다.
/// 전송만 `HidDevice`(IOKit)로 교체했다.
final class HidppDevice {
    static let reportIdLong: UInt8 = 0x11
    static let reportIdShort: UInt8 = 0x10

    // 응답 매칭용 software id (0 아닌 4비트 값).
    private static let softwareId: UInt8 = 0x0A

    // 기능/인덱스 상수.
    static let iRootIndex: UInt8 = 0x00
    static let featureChangeHost: UInt16 = 0x1814
    static let featureDeviceName: UInt16 = 0x0005

    private let stream: HidDevice
    private let log: DiagLogger

    init(stream: HidDevice, log: DiagLogger) {
        self.stream = stream
        self.log = log
    }

    private static func funcByte(_ funcIndex: UInt8) -> UInt8 {
        return UInt8(((funcIndex & 0x0F) << 4) | softwareId)
    }

    /// 출력 리포트 길이로 long(0x11·20바이트)/short(0x10·7바이트) 프레임을 만든다.
    private func buildReport(deviceIndex: UInt8, featureIndex: UInt8, funcIndex: UInt8, parms: [UInt8]?) -> [UInt8] {
        let useLong = stream.outputReportByteLength >= 20
        let reportLen = useLong ? 20 : 7
        let maxParams = useLong ? 16 : 3

        var report = [UInt8](repeating: 0, count: reportLen)
        report[0] = useLong ? HidppDevice.reportIdLong : HidppDevice.reportIdShort
        report[1] = deviceIndex
        report[2] = featureIndex
        report[3] = HidppDevice.funcByte(funcIndex)
        if let parms = parms {
            let n = min(parms.count, maxParams)
            for i in 0..<n { report[4 + i] = parms[i] }
        }
        return report
    }

    /// HID++ 리포트로 호출하고 응답 payload(파라미터 16바이트)를 돌려준다.
    /// 타임아웃 시 nil, 오류 응답 시 HidppError throw.
    func call(deviceIndex: UInt8, featureIndex: UInt8, funcIndex: UInt8, parms: [UInt8]?, timeoutMs: Int) throws -> [UInt8]? {
        let funcByteVal = HidppDevice.funcByte(funcIndex)
        let report = buildReport(deviceIndex: deviceIndex, featureIndex: featureIndex, funcIndex: funcIndex, parms: parms)

        log.logTx(deviceIndex: deviceIndex, report: report)

        if !stream.write(report, timeoutMs: timeoutMs) {
            log.logTimeout(String(format: "Write dev=0x%02X feat=0x%02X func=%d", deviceIndex, featureIndex, funcIndex))
            return nil
        }

        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000.0)
        while true {
            let remaining = Int(deadline.timeIntervalSinceNow * 1000.0)
            if remaining <= 0 {
                log.logTimeout(String(format: "Read dev=0x%02X feat=0x%02X func=%d", deviceIndex, featureIndex, funcIndex))
                return nil
            }

            guard let resp = stream.read(timeoutMs: remaining) else {
                log.logTimeout(String(format: "Read dev=0x%02X feat=0x%02X func=%d", deviceIndex, featureIndex, funcIndex))
                return nil
            }

            log.logRx(resp)

            if resp.count < 4 {
                continue
            }

            let rId = resp[0]
            let rDev = resp[1]
            let rFeat = resp[2]
            let rFunc = resp[3]

            // HID++ 2.0 오류 응답: feature index 자리에 0xFF.
            if (rId == HidppDevice.reportIdLong || rId == HidppDevice.reportIdShort) && rFeat == 0xFF
                && rDev == deviceIndex && resp.count >= 6
                && resp[3] == featureIndex && resp[4] == funcByteVal {
                let code = Int(resp[5])
                throw HidppError(code: code, message: "HID++2.0 " + HidppError.describe(code))
            }

            // HID++ 1.0 오류 응답(리시버): 0x8F.
            if rId == HidppDevice.reportIdShort && rFeat == 0x8F && rDev == deviceIndex {
                let code = resp.count >= 6 ? Int(resp[5]) : -1
                throw HidppError(code: code, message: String(format: "HID++1.0 error 0x%02X", code))
            }

            // 정상 응답 매칭.
            if rDev == deviceIndex && rFeat == featureIndex && rFunc == funcByteVal {
                var payload = [UInt8](repeating: 0, count: 16)
                let copy = min(16, resp.count - 4)
                if copy > 0 {
                    for i in 0..<copy { payload[i] = resp[4 + i] }
                }
                return payload
            }

            // 그 밖은 비동기 알림 — 무시하고 계속 읽는다.
        }
    }

    /// IRoot ping(func1): 응답 echo 로 장치 존재 확인.
    func ping(deviceIndex: UInt8, timeoutMs: Int) -> Bool {
        let marker: UInt8 = 0x5A
        let payload: [UInt8]?
        do {
            payload = try call(deviceIndex: deviceIndex, featureIndex: HidppDevice.iRootIndex, funcIndex: 1,
                               parms: [0x00, 0x00, marker], timeoutMs: timeoutMs)
        } catch is HidppError {
            // 오류 응답도 "장치가 응답했다"는 증거다.
            return true
        } catch {
            return false
        }
        guard let payload = payload else { return false }
        return payload[2] == marker
    }

    /// IRoot getFeature(func0): featureId 의 런타임 인덱스. 0 이면 미지원.
    func getFeatureIndex(deviceIndex: UInt8, featureId: UInt16, timeoutMs: Int) throws -> UInt8 {
        let parms: [UInt8] = [UInt8(featureId >> 8), UInt8(featureId & 0xFF)]
        guard let payload = try call(deviceIndex: deviceIndex, featureIndex: HidppDevice.iRootIndex, funcIndex: 0,
                                     parms: parms, timeoutMs: timeoutMs) else {
            return 0
        }
        return payload[0]
    }

    /// 0x1814 func0 getHostInfo → (호스트 개수, 현재 호스트 0-base).
    func getHostInfo(deviceIndex: UInt8, changeHostIndex: UInt8, timeoutMs: Int) throws -> (nbHost: Int, currHost: Int)? {
        guard let payload = try call(deviceIndex: deviceIndex, featureIndex: changeHostIndex, funcIndex: 0,
                                     parms: nil, timeoutMs: timeoutMs) else {
            return nil
        }
        return (Int(payload[0]), Int(payload[1]))
    }

    /// 0x0005 로 기기 이름을 읽는다(best-effort). 실패 시 nil.
    func getDeviceName(deviceIndex: UInt8, nameFeatureIndex: UInt8, timeoutMs: Int) -> String? {
        do {
            // getCount
            guard let head = try call(deviceIndex: deviceIndex, featureIndex: nameFeatureIndex, funcIndex: 0,
                                      parms: nil, timeoutMs: timeoutMs) else {
                return nil
            }
            let count = Int(head[0])
            if count <= 0 || count > 64 {
                return nil
            }

            var bytes = [UInt8]()
            var idx = 0
            var guardCount = 0
            while idx < count && guardCount < 16 {
                guardCount += 1
                // getName(idx)
                guard let chunk = try call(deviceIndex: deviceIndex, featureIndex: nameFeatureIndex, funcIndex: 1,
                                           parms: [UInt8(idx)], timeoutMs: timeoutMs) else {
                    break
                }
                var added = 0
                var i = 0
                while i < chunk.count && idx < count {
                    let b = chunk[i]
                    if b == 0 { break }
                    if b >= 0x20 && b < 0x7F {
                        bytes.append(b)
                    }
                    idx += 1
                    added += 1
                    i += 1
                }
                if added == 0 { break }
            }
            let name = String(bytes: bytes, encoding: .ascii)?.trimmingCharacters(in: .whitespaces) ?? ""
            return name.isEmpty ? nil : name
        } catch is HidppError {
            return nil
        } catch {
            return nil
        }
    }
}
