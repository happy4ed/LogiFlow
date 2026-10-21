// swift-tools-version:5.7
import PackageDescription

// LogiFlow macOS 진단 도구. 윈도우 버전의 HID++ 프로토콜 로직을 그대로 옮기고
// 전송 계층만 IOKit(IOHIDManager)로 교체한 읽기 전용 진단 앱.
let package = Package(
    name: "LogiFlowDiag",
    platforms: [
        .macOS(.v12)
    ],
    targets: [
        .executableTarget(
            name: "LogiFlowDiag",
            path: "Sources/LogiFlowDiag"
        )
    ]
)
