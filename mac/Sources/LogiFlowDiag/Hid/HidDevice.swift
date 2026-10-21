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
    func open() -> Bool {
        let r = IOHIDDeviceOpen(device, IOOptionBits(kIOHIDOptionsTypeNone))
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

    /// 입력 리포트 수신 콜백(HidRunLoop 스레드에서 실행). macOS 는 reportID 를 별도 인자로
    /// 주고 버퍼에는 ID 를 뺀 데이터만 담으므로, 윈도우 프레임과 동일하게 [ID]+데이터로 복원한다.
    private func onInput(reportID: UInt32, report: UnsafeMutablePointer<UInt8>?, length: CFIndex) {
        var frame = [UInt8]()
        frame.append(UInt8(reportID & 0xFF))
        if let report = report, length > 0 {
            frame.append(contentsOf: UnsafeBufferPointer(start: report, count: Int(length)))
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
        IOHIDDeviceClose(device, IOOptionBits(kIOHIDOptionsTypeNone))
        opened = false
    }
}

/// IOHIDDevice 입력 콜백이 발화하려면 전용 CFRunLoop 가 돌아야 한다.
/// 백그라운드 스레드 1개에서 런루프를 돌리고, 열린 기기를 거기에 스케줄한다.
final class HidRunLoop {
    static let shared = HidRunLoop()

    private var runLoop: CFRunLoop!
    private let ready = DispatchSemaphore(value: 0)
    let mode = CFRunLoopMode.defaultMode

    private init() {
        let thread = Thread { [weak self] in
            guard let self = self else { return }
            self.runLoop = CFRunLoopGetCurrent()
            self.ready.signal()
            // 빈 런루프는 소스가 없으면 즉시 반환하므로, 깨어 있게 타이머 소스를 하나 건다.
            let timer = CFRunLoopTimerCreate(kCFAllocatorDefault, CFAbsoluteTimeGetCurrent() + 1,
                                             1, 0, 0, { _, _ in }, nil)
            CFRunLoopAddTimer(self.runLoop, timer, CFRunLoopMode.defaultMode)
            CFRunLoopRun()
        }
        thread.name = "LogiFlow.HidRunLoop"
        thread.start()
        ready.wait()
    }

    func schedule(_ device: IOHIDDevice) {
        IOHIDDeviceScheduleWithRunLoop(device, runLoop, mode.rawValue)
    }

    func unschedule(_ device: IOHIDDevice) {
        IOHIDDeviceUnscheduleFromRunLoop(device, runLoop, mode.rawValue)
    }
}
