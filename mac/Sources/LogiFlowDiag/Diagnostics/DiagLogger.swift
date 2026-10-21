import Foundation

/// 진단 로그. 보낸 요청/받은 응답 raw 바이트(hex)·타임아웃·오류를
/// ~/Library/Logs/LogiFlow/logiflow-diag.log 에 남긴다. serial·IP 등 민감값은 기록하지 않는다.
/// 윈도우 `DiagLogger` 대응.
final class DiagLogger {
    private let lock = NSLock()
    private var memory = ""

    let logDirectory: URL
    let logPath: URL

    /// 진단 로그 남기기(설정). 꺼도 메모리 스냅샷은 유지.
    var enabled = true

    init() {
        let home = FileManager.default.homeDirectoryForCurrentUser
        logDirectory = home.appendingPathComponent("Library/Logs/LogiFlow", isDirectory: true)
        logPath = logDirectory.appendingPathComponent("logiflow-diag.log")
    }

    static func hex(_ data: [UInt8]?) -> String {
        guard let data = data else { return "(nil)" }
        return data.map { String(format: "%02X", $0) }.joined(separator: " ")
    }

    func log(_ line: String) {
        let fmt = DateFormatter()
        fmt.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        let stamped = "\(fmt.string(from: Date()))  \(line)"
        lock.lock()
        defer { lock.unlock() }
        memory += stamped + "\n"
        if !enabled { return }  // 파일 기록 끔(메모리 사본만).
        do {
            try FileManager.default.createDirectory(at: logDirectory, withIntermediateDirectories: true)
            let line = stamped + "\n"
            if let handle = try? FileHandle(forWritingTo: logPath) {
                handle.seekToEndOfFile()
                handle.write(line.data(using: .utf8)!)
                handle.closeFile()
            } else {
                try line.data(using: .utf8)!.write(to: logPath)
            }
        } catch {
            // 파일 기록 실패는 조용히 무시 — 메모리 사본은 유지.
        }
    }

    func logTx(deviceIndex: UInt8, report: [UInt8]) {
        log(String(format: "TX  dev=0x%02X  [%@]", deviceIndex, DiagLogger.hex(report)))
    }

    func logRx(_ report: [UInt8]) {
        log("RX  [\(DiagLogger.hex(report))]")
    }

    func logTimeout(_ context: String) {
        log("TIMEOUT  " + context)
    }

    func logError(_ context: String) {
        log("ERROR  " + context)
    }

    func snapshot() -> String {
        lock.lock()
        defer { lock.unlock() }
        return memory
    }
}
