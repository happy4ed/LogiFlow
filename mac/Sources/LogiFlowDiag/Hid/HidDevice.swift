import Foundation
import IOKit
import IOKit.hid

/// IOHIDDevice 하나에 대한 읽기/쓰기(타임아웃 지원). 윈도우 `HidStream` 의 macOS 대응.
///
/// 전송 계층만 IOKit 으로 교체했다:
///  - 쓰기: `IOHIDDeviceSetReport(output)` — HID++ 요청 리포트 송신.
///  - 읽기: `IOHIDDeviceRegisterInputReportCallback` 로 들어온 입력 리포트를 큐에 쌓고,
///          `read(timeoutMs:)` 가 조건변수로 기다렸다가 1건을 꺼낸다(윈도우의 타임아웃 Read 대응).
///
/// 보내는 명령은 조회성(func0 / IRoot 0x0000 / 0x0005) 뿐이며 쓰기 명령(setHost)은 보내지 않는다.
final class HidDevice {
    let info: HidInterfaceInfo

    private let device: IOHIDDevice
    private let bufLen: Int
    private let reportBuffer: UnsafeMutablePointer<UInt8>
    private let cond = NSCondition()
    private var frames: [[UInt8]] = []
    private var opened = false

    var outputReportByteLength: Int { info.outputReportByteLength }
    var inputReportByteLength: Int { info.inputReportByteLength }

    init(device: IOHIDDevice, info: HidInterfaceInfo) {
        self.device = device
        self.info = info
        // 입력 콜백 버퍼: 최대 입력 리포트(ID 제외) 이상. 넉넉히 64 바이트 보장.
        let maxIn = max(info.inputReportByteLength, 64)
        self.bufLen = maxIn
        self.reportBuffer = UnsafeMutablePointer<UInt8>.allocate(capacity: maxIn)
        self.reportBuffer.initialize(repeating: 0, count: maxIn)
    }

    deinit {
        reportBuffer.deinitialize(count: bufLen)
        reportBuffer.deallocate()
    }

    /// 공유 모드(seize 아님)로 연다 — Logi Options+ 등 다른 앱과 충돌하지 않게.
    /// 옵션 0 = kIOHIDOptionsTypeNone(공유). seize(0x01)는 쓰지 않는다.
    func open() -> Bool {
        let r = IOHIDDeviceOpen(device, 0)
        guard r == kIOReturnSuccess else { return false }

        let ctx = Unmanaged.passUnretained(self).toOpaque()
        IOHIDDeviceRegisterInputReportCallback(
            device, reportBuffer, bufLen,
            { context, _, _, _, reportID, report, reportLength in
                guard let context = context else { return }
                let me = Unmanaged<HidDevice>.fromOpaque(context).takeUnretainedValue()
                me.onInput(reportID: reportID, report: report, length: reportLength)
            },
            ctx)

        HidRunLoop.shared.schedule(device)
        opened = true
        return true
    }

    /// 입력 리포트 수신 콜백(HidRunLoop 스레드에서 실행).
    /// 윈도우 프레임과 동일하게 **항상 [리포트ID]+데이터** 꼴로 복원한다(resp[0]=ID, resp[1]=deviceIndex…).
    ///
    /// macOS IOKit 버전/기기에 따라 콜백 버퍼가 리포트 ID 를 "포함"하거나 "제외"한다(출처가 갈림).
    /// HID++ 리포트 ID 는 0x10/0x11 뿐이고 deviceIndex 는 0x01~0x06·0xFF 뿐이라 겹치지 않으므로:
    ///  - 버퍼 첫 바이트가 0x10/0x11 이면 이미 ID 포함 → 그대로 사용.
    ///  - 아니면 ID 가 빠진 것 → 콜백의 reportID 를 앞에 붙인다.
    private func onInput(reportID: UInt32, report: UnsafeMutablePointer<UInt8>?, length: CFIndex) {
        var data = [UInt8]()
        if let report = report, length > 0 {
            data = Array(UnsafeBufferPointer(start: report, count: Int(length)))
        }
        var frame: [UInt8]
        if let first = data.first, first == HidppDevice.reportIdLong || first == HidppDevice.reportIdShort {
            frame = data                       // 이미 ID 포함.
        } else {
            frame = [UInt8(reportID & 0xFF)] + data   // ID 선두에 복원.
        }
        cond.lock()
        frames.append(frame)
        cond.signal()
        cond.unlock()
    }

    /// 출력 리포트 전송. report[0] 은 리포트 ID(0x10/0x11). hidapi 와 동일하게 ID 를 포함한
    /// 전체 버퍼를 보내고 reportID 인자에도 report[0] 을 넘긴다.
    func write(_ report: [UInt8], timeoutMs: Int) -> Bool {
        guard opened, !report.isEmpty else { return false }
        let reportID = CFIndex(report[0])
        let r = report.withUnsafeBufferPointer { buf -> IOReturn in
            guard let base = buf.baseAddress else { return kIOReturnBadArgument }
            return IOHIDDeviceSetReport(device, kIOHIDReportTypeOutput, reportID, base, report.count)
        }
        return r == kIOReturnSuccess
    }

    /// 입력 리포트 1건 수신. 타임아웃 시 nil.
    func read(timeoutMs: Int) -> [UInt8]? {
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000.0)
        cond.lock()
        defer { cond.unlock() }
        while frames.isEmpty {
            let remaining = deadline.timeIntervalSinceNow
            if remaining <= 0 { return nil }
            _ = cond.wait(until: Date().addingTimeInterval(remaining))
            if frames.isEmpty && Date() >= deadline { return nil }
        }
        return frames.isEmpty ? nil : frames.removeFirst()
    }

    func close() {
        guard opened else { return }
        HidRunLoop.shared.unschedule(device)
        IOHIDDeviceClose(device, 0)
        opened = false
    }
}

/// IOHIDDevice 입력 콜백이 발화하려면 전용 CFRunLoop 가 돌아야 한다.
/// 백그라운드 스레드 1개에서 런루프를 돌리고, 열린 기기를 거기에 스케줄한다.
final class HidRunLoop {
    static let shared = HidRunLoop()

    private var runLoop: CFRunLoop!
    private let ready = DispatchSemaphore(value: 0)
    let mode = CFRunLoopMode.defaultMode!

    private init() {
        // 수명 싱글턴 — 강한 캡처로 초기화 중 nil 위험을 없앤다(순환은 앱 수명 동안 의도된 것).
        let thread = Thread {
            self.runLoop = CFRunLoopGetCurrent()
            self.ready.signal()
            // 빈 런루프는 소스가 없으면 즉시 반환하므로, 깨어 있게 타이머 소스를 하나 건다.
            let timer = CFRunLoopTimerCreate(kCFAllocatorDefault, CFAbsoluteTimeGetCurrent() + 1,
                                             1, 0, 0, { _, _ in }, nil)
            CFRunLoopAddTimer(self.runLoop, timer, self.mode)
            CFRunLoopRun()
        }
        thread.name = "LogiFlow.HidRunLoop"
        thread.start()
        ready.wait()
    }

    func schedule(_ device: IOHIDDevice) {
        IOHIDDeviceScheduleWithRunLoop(device, runLoop, mode.rawValue)
        // 다른 스레드에서 소스를 추가했으니 런루프를 깨워 즉시 반영되게 한다.
        CFRunLoopWakeUp(runLoop)
    }

    func unschedule(_ device: IOHIDDevice) {
        IOHIDDeviceUnscheduleFromRunLoop(device, runLoop, mode.rawValue)
    }
}
