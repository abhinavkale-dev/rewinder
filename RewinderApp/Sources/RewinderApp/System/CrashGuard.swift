import Foundation
import Darwin

enum CrashGuard {
    private nonisolated(unsafe) static var ffmpegPidPath: UnsafeMutablePointer<CChar>?
    private nonisolated(unsafe) static var helperPidPath: UnsafeMutablePointer<CChar>?
    private nonisolated(unsafe) static var crashLogPath: UnsafeMutablePointer<CChar>?
    private nonisolated(unsafe) static var installed = false

    private nonisolated(unsafe) static let signalMarker: [UInt8] =
        Array("rewinder: fatal signal — capture children terminated\n".utf8)

    static func updatePaths(outputDir: String?) {
        guard let dir = outputDir, !dir.isEmpty else { return }
        let live = (dir as NSString).appendingPathComponent(".rewinder-live")
        ffmpegPidPath = strdup((live as NSString).appendingPathComponent("ffmpeg-capture.pid"))
        helperPidPath = strdup((live as NSString).appendingPathComponent("sck-capture.pid"))
        crashLogPath = strdup((live as NSString).appendingPathComponent("crash.log"))
    }

    static func install() {
        guard !installed else { return }
        installed = true

        NSSetUncaughtExceptionHandler { exception in
            CrashGuard.handleUncaughtException(exception)
        }

        let fatalSignals: [Int32] = [
            SIGTERM, SIGINT, SIGSEGV, SIGABRT, SIGBUS, SIGILL, SIGFPE, SIGTRAP,
        ]
        for sig in fatalSignals {
            signal(sig, rewinderSignalHandler)
        }
    }

    static func handleSignal(_ sig: Int32) {
        writeSignalMarker()
        emergencyKillChildren()
        signal(sig, SIG_DFL)
        raise(sig)
    }

    static func emergencyKillChildren() {
        killFromPidFile(ffmpegPidPath)
        killFromPidFile(helperPidPath)
    }

    private static func killFromPidFile(_ path: UnsafeMutablePointer<CChar>?) {
        guard let path else { return }
        let fd = open(path, O_RDONLY)
        if fd < 0 { return }
        var buf = [CChar](repeating: 0, count: 32)
        let n = read(fd, &buf, 31)
        close(fd)
        if n <= 0 { return }
        var pid: Int32 = 0
        for i in 0..<Int(n) {
            let c = buf[i]
            if c >= 48 && c <= 57 {
                pid = pid * 10 + Int32(c - 48)
            } else {
                break
            }
        }
        if pid > 0 {
            kill(pid_t(pid), SIGKILL)
        }
    }

    private static func writeSignalMarker() {
        guard let path = crashLogPath else { return }
        let fd = open(path, O_WRONLY | O_CREAT | O_APPEND, 0o644)
        if fd < 0 { return }
        signalMarker.withUnsafeBytes { raw in
            if let base = raw.baseAddress { _ = write(fd, base, raw.count) }
        }
        close(fd)
    }

    static func handleUncaughtException(_ exception: NSException) {
        let name = exception.name.rawValue
        let reason = exception.reason ?? "(no reason)"
        let stack = exception.callStackSymbols.joined(separator: "\n")
        let entry = "=== Uncaught exception \(Date()) ===\n\(name): \(reason)\n\(stack)\n\n"
        appendCrashLog(entry)
        emergencyKillChildren()
    }

    private static func appendCrashLog(_ text: String) {
        guard let path = crashLogPath else { return }
        let file = String(cString: path)
        let dir = (file as NSString).deletingLastPathComponent
        try? FileManager.default.createDirectory(
            atPath: dir, withIntermediateDirectories: true)
        guard let data = text.data(using: .utf8) else { return }
        if let handle = try? FileHandle(forWritingTo: URL(fileURLWithPath: file)) {
            defer { try? handle.close() }
            handle.seekToEndOfFile()
            handle.write(data)
        } else {
            try? text.write(toFile: file, atomically: true, encoding: .utf8)
        }
    }
}

private func rewinderSignalHandler(_ sig: Int32) {
    CrashGuard.handleSignal(sig)
}
