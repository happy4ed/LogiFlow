import Foundation

/// 열거된 HID 인터페이스(= 하나의 top-level 컬렉션) 1개의 정적 속성.
/// 윈도우 `HidInterfaceInfo` 와 같은 역할. macOS 에서는 하나의 물리 기기가
/// 컬렉션(long 0x0202 / short 0x0201)마다 별도의 IOHIDDevice 로 열거된다.
///
/// 리포트 길이 규약: 윈도우 WinHID 의 Output/InputReportByteLength 는 리포트 ID 바이트를
/// 포함한다(long=20, short=7). macOS 의 kIOHIDMax{Input,Output}ReportSize 는 ID 바이트를
/// 제외한 값이므로, 윈도우와 동일한 의미(= ID 포함 길이)가 되도록 열거 시 +1 해서 저장한다.
/// 그래야 포팅한 `HidppDevice.buildReport` 의 `outputReportByteLength >= 20` long/short
/// 선택 로직을 손대지 않고 그대로 쓸 수 있다.
final class HidInterfaceInfo {
    var path: String = ""              // 합성 식별자(locationID + usage). 변화 감지·중복 제거용
    var vendorId: UInt16 = 0
    var productId: UInt16 = 0
    var usagePage: UInt16 = 0          // PrimaryUsagePage
    var usage: UInt16 = 0              // PrimaryUsage
    var inputReportByteLength: Int = 0   // ID 포함(윈도우 의미)
    var outputReportByteLength: Int = 0  // ID 포함(윈도우 의미)
    var productString: String? = nil
    var transport: String? = nil       // USB / Bluetooth 등
    var capsRead: Bool = false          // caps/attrs 를 실제로 읽었는지(false면 경로에서만 추출)

    var vidPid: String {
        return String(format: "%04X:%04X", vendorId, productId)
    }
}
