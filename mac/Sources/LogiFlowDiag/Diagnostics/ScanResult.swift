import Foundation
import IOKit.hid

/// 발견된 HID++ 장치 1개(= 인터페이스 × deviceIndex)의 진단 결과.
/// 윈도우 `ScanResult` 대응. 전환(setHost) 때 같은 인터페이스를 다시 열도록 IOHIDDevice 를 들고 있다.
final class ScanResult: Identifiable {
    let id = UUID()
    var interface: HidInterfaceInfo? = nil
    var device: IOHIDDevice? = nil
    var deviceName: String? = nil      // HID++ 0x0005 또는 제품 문자열
    var vidPid: String = ""            // 046D:XXXX
    var path: String = ""              // 합성 식별자
    var connectionType: String? = nil  // 직접(USB/BT) / 리시버 뒤(index N)
    var deviceIndex: UInt8 = 0         // 0xFF 또는 1~6
    var changeHostSupported: Bool = false  // 0x1814 지원 여부
    var changeHostIndex: UInt8 = 0     // 런타임 조회된 0x1814 feature 인덱스
    var hostCount: Int = -1            // 호스트(채널) 개수
    var currentHost: Int = -1          // 현재 호스트(0-base)
    var note: String? = nil            // 비고(오류·미지원 등)
}
