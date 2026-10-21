import Foundation
import IOKit
import IOKit.hid

/// IOHIDManager 로 시스템의 HID 인터페이스를 열거한다. 읽기 전용.
/// 윈도우 `HidEnumerator`(SetupAPI) 의 macOS 대응.
enum HidManager {

    /// VID 필터로 HID 인터페이스를 전부 가져온다. vendorIdFilter 0 이면 전체 VID.
    /// usagePageFilter 0 이면 전체 usagePage.
    static func enumerate(vendorIdFilter: UInt16, usagePageFilter: UInt16) -> [HidInterfaceInfo] {
        var result = [HidInterfaceInfo]()

        let mgr = IOHIDManagerCreate(kCFAllocatorDefault, IOOptionBits(kIOHIDOptionsTypeNone))
        // 매칭 딕셔너리로 VID 만 1차 필터(나머지는 코드에서).
        if vendorIdFilter != 0 {
            let match: [String: Any] = [kIOHIDVendorIDKey: Int(vendorIdFilter)]
            IOHIDManagerSetDeviceMatching(mgr, match as CFDictionary)
        } else {
            IOHIDManagerSetDeviceMatching(mgr, nil)
        }

        guard let deviceSet = IOHIDManagerCopyDevices(mgr) as? Set<IOHIDDevice> else {
            return result
        }

        for device in deviceSet {
            let info = readInterface(device)
            if vendorIdFilter != 0 && info.vendorId != 0 && info.vendorId != vendorIdFilter {
                continue
            }
            if usagePageFilter != 0 && info.usagePage != usagePageFilter {
                continue
            }
            result.append(info)
        }
        return result
    }

    /// 매칭된 IOHIDDevice 를 다시 열어 I/O 가능한 HidDevice 로 만든다.
    /// 열거에서 쓴 IOHIDDevice 참조를 그대로 재사용한다.
    static func openDevices(vendorIdFilter: UInt16) -> [(HidInterfaceInfo, IOHIDDevice)] {
        var out = [(HidInterfaceInfo, IOHIDDevice)]()
        let mgr = IOHIDManagerCreate(kCFAllocatorDefault, IOOptionBits(kIOHIDOptionsTypeNone))
        if vendorIdFilter != 0 {
            IOHIDManagerSetDeviceMatching(mgr, [kIOHIDVendorIDKey: Int(vendorIdFilter)] as CFDictionary)
        } else {
            IOHIDManagerSetDeviceMatching(mgr, nil)
        }
        guard let deviceSet = IOHIDManagerCopyDevices(mgr) as? Set<IOHIDDevice> else { return out }
        for device in deviceSet {
            let info = readInterface(device)
            if vendorIdFilter != 0 && info.vendorId != 0 && info.vendorId != vendorIdFilter { continue }
            out.append((info, device))
        }
        return out
    }

    private static func intProp(_ device: IOHIDDevice, _ key: String) -> Int? {
        guard let ref = IOHIDDeviceGetProperty(device, key as CFString) else { return nil }
        if CFGetTypeID(ref) == CFNumberGetTypeID() {
            var value = 0
            CFNumberGetValue((ref as! CFNumber), .sInt64Type, &value)
            return value
        }
        return nil
    }

    private static func stringProp(_ device: IOHIDDevice, _ key: String) -> String? {
        guard let ref = IOHIDDeviceGetProperty(device, key as CFString) else { return nil }
        if CFGetTypeID(ref) == CFStringGetTypeID() {
            return (ref as! CFString) as String
        }
        return nil
    }

    private static func readInterface(_ device: IOHIDDevice) -> HidInterfaceInfo {
        let info = HidInterfaceInfo()
        info.vendorId = UInt16(intProp(device, kIOHIDVendorIDKey) ?? 0)
        info.productId = UInt16(intProp(device, kIOHIDProductIDKey) ?? 0)
        info.usagePage = UInt16(intProp(device, kIOHIDPrimaryUsagePageKey) ?? 0)
        info.usage = UInt16(intProp(device, kIOHIDPrimaryUsageKey) ?? 0)
        // macOS 의 Max*ReportSize 는 ID 바이트를 제외 → 윈도우 의미(ID 포함)에 맞춰 +1.
        if let mi = intProp(device, kIOHIDMaxInputReportSizeKey), mi > 0 {
            info.inputReportByteLength = mi + 1
        }
        if let mo = intProp(device, kIOHIDMaxOutputReportSizeKey), mo > 0 {
            info.outputReportByteLength = mo + 1
        }
        info.productString = stringProp(device, kIOHIDProductKey)
        info.transport = stringProp(device, kIOHIDTransportKey)
        info.capsRead = info.usagePage != 0 || info.inputReportByteLength > 0

        // 합성 경로(locationID + usage) — 윈도우의 디바이스 경로 대용.
        let loc = intProp(device, kIOHIDLocationIDKey) ?? 0
        info.path = String(format: "loc=%08X:up=%04X:u=%04X:pid=%04X",
                           loc, info.usagePage, info.usage, info.productId)
        return info
    }
}
