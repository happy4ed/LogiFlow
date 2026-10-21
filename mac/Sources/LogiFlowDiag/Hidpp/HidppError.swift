import Foundation

/// HID++ 장치가 반환한 오류 응답. 윈도우 `HidppErrorException` 대응.
struct HidppError: Error {
    let code: Int
    let message: String

    init(code: Int, message: String) {
        self.code = code
        self.message = message
    }

    static func describe(_ code: Int) -> String {
        switch code {
        case 0: return "NoError"
        case 1: return "Unknown"
        case 2: return "InvalidArgument"
        case 3: return "OutOfRange"
        case 4: return "HardwareError"
        case 5: return "LogitechInternal"
        case 6: return "InvalidFeatureIndex"
        case 7: return "InvalidFunctionId"
        case 8: return "Busy"
        case 9: return "Unsupported"
        default: return String(format: "Error(0x%02X)", code)
        }
    }
}
