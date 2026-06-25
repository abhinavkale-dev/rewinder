import AVFoundation
import Darwin
import Foundation

@main
struct RewinderSCKCapture {
    struct MicrophoneDeviceInfo: Codable {
        let id: String
        let name: String
        let isDefault: Bool
        let isAvailable: Bool
    }

    static func main() {
        if CommandLine.arguments.contains("--probe-mic-permission") ||
            CommandLine.arguments.contains("--request-mic-permission")
        {
            let request = CommandLine.arguments.contains("--request-mic-permission")
            let granted = probeMicrophonePermission(requestIfNeeded: request)
            exit(granted ? 0 : 2)
        }

        if CommandLine.arguments.contains("--list-microphones") {
            do {
                try listMicrophones()
                exit(0)
            } catch {
                fputs("ScreenCaptureKit helper failed: \(error)\n", stderr)
                fflush(stderr)
                exit(1)
            }
        }

        let config: CaptureConfig
        do {
            config = try parseConfig()
        } catch {
            fputs("ScreenCaptureKit helper failed: \(error)\n", stderr)
            fflush(stderr)
            exit(1)
        }

        let controller = CaptureController(config: config)

        signal(SIGTERM, SIG_IGN)
        signal(SIGINT, SIG_IGN)

        let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
        term.setEventHandler {
            controller.stopAndExit(0)
        }
        term.resume()

        let interrupt = DispatchSource.makeSignalSource(signal: SIGINT, queue: .main)
        interrupt.setEventHandler {
            controller.stopAndExit(0)
        }
        interrupt.resume()

        let initialParentPID = getppid()
        let parentWatchdog = DispatchSource.makeTimerSource(queue: .main)
        parentWatchdog.schedule(deadline: .now() + 1.0, repeating: 1.0)
        parentWatchdog.setEventHandler {
            if getppid() != initialParentPID {
                fputs("phase: helper_parent_lost initial_ppid=\(initialParentPID)\n", stderr)
                fflush(stderr)
                controller.stopAndExit(0)
            }
        }
        parentWatchdog.resume()

        let parentExitSource: DispatchSourceProcess?
        if let parentPID = config.parentPID, parentPID > 1 {
            let watchdogQueue = DispatchQueue(label: "com.rewinder.sck.parent-watchdog")
            let source = DispatchSource.makeProcessSource(
                identifier: parentPID, eventMask: .exit, queue: watchdogQueue
            )
            source.setEventHandler {
                fputs("phase: helper_parent_exited parent_pid=\(parentPID)\n", stderr)
                fflush(stderr)
                if let ffmpegPID = config.ffmpegPID, ffmpegPID > 1 {
                    kill(ffmpegPID, SIGKILL)
                }
                _exit(0)
            }
            source.resume()
            parentExitSource = source
        } else {
            parentExitSource = nil
        }
        _ = parentExitSource

        Task { @MainActor in
            do {
                try await controller.start()
            } catch {
                fputs("ScreenCaptureKit helper failed: \(error)\n", stderr)
                fflush(stderr)
                exit(1)
            }
        }
        RunLoop.main.run()
    }

    private static func listMicrophones() throws {
        let defaultID = AVCaptureDevice.default(for: .audio)?.uniqueID
        let devices = AVCaptureDevice.devices(for: .audio).map { device in
            MicrophoneDeviceInfo(
                id: device.uniqueID,
                name: device.localizedName,
                isDefault: device.uniqueID == defaultID,
                isAvailable: true
            )
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(devices)
        FileHandle.standardOutput.write(data)
    }

    private static func probeMicrophonePermission(requestIfNeeded: Bool) -> Bool {
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        switch status {
        case .authorized:
            fputs("mic_permission=granted\n", stderr)
            fflush(stderr)
            return true
        case .denied:
            fputs("mic_permission=denied\n", stderr)
            fflush(stderr)
            return false
        case .restricted:
            fputs("mic_permission=restricted\n", stderr)
            fflush(stderr)
            return false
        case .notDetermined:
            if !requestIfNeeded {
                fputs("mic_permission=not_determined\n", stderr)
                fflush(stderr)
                return false
            }

            let semaphore = DispatchSemaphore(value: 0)
            var granted = false
            AVCaptureDevice.requestAccess(for: .audio) { access in
                granted = access
                semaphore.signal()
            }
            _ = semaphore.wait(timeout: .now() + 10)
            fputs("mic_permission=\(granted ? "granted" : "denied")\n", stderr)
            fflush(stderr)
            return granted
        @unknown default:
            fputs("mic_permission=unknown\n", stderr)
            fflush(stderr)
            return false
        }
    }

    private static func parseConfig() throws -> CaptureConfig {
        let args = CommandLine.arguments

        func value(for key: String) -> String? {
            guard let idx = args.firstIndex(of: key), idx + 1 < args.count else {
                return nil
            }
            return args[idx + 1]
        }

        guard
            let widthRaw = value(for: "--width"),
            let heightRaw = value(for: "--height"),
            let fpsRaw = value(for: "--fps"),
            let videoPipe = value(for: "--video-pipe"),
            let width = Int(widthRaw),
            let height = Int(heightRaw),
            let fps = Int(fpsRaw)
        else {
            throw CaptureError.invalidArgs("expected --width --height --fps --video-pipe")
        }

        let displayIndex = Int(value(for: "--display-index") ?? "0") ?? 0
        let displayID = value(for: "--display-id").flatMap { CGDirectDisplayID($0) }
        let enableSystemAudio = (value(for: "--enable-system-audio") ?? "0") == "1"
        let enableMic = (value(for: "--enable-mic") ?? "0") == "1"
        let audioSampleRate = Int(value(for: "--audio-sample-rate") ?? "48000") ?? 48_000
        let audioChannels = Int(value(for: "--audio-channels") ?? "2") ?? 2
        let excludeCurrentProcessAudio = (value(for: "--exclude-current-process-audio") ?? "1") != "0"
        let micBackend = value(for: "--mic-backend") ?? "avcapture"
        let selectedMicrophoneID = value(for: "--selected-microphone-id").flatMap { raw in
            raw.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : raw
        }
        let micRetryIntervalSecs = Int(value(for: "--mic-retry-interval-secs") ?? "15") ?? 15
        let boostMicVolume = (value(for: "--boost-mic-volume") ?? "0") == "1"
        let parentPID = value(for: "--parent-pid").flatMap { pid_t($0) }
        let ffmpegPID = value(for: "--ffmpeg-pid").flatMap { pid_t($0) }

        let audioPipe = value(for: "--audio-pipe")
        let micPipe = value(for: "--mic-pipe")

        if enableSystemAudio && audioPipe == nil {
            throw CaptureError.invalidArgs("--audio-pipe is required when --enable-system-audio=1")
        }
        if enableMic && micPipe == nil {
            throw CaptureError.invalidArgs("--mic-pipe is required when --enable-mic=1")
        }

        return CaptureConfig(
            width: width,
            height: height,
            fps: max(fps, 1),
            displayIndex: max(displayIndex, 0),
            displayID: displayID,
            videoPipe: videoPipe,
            audioPipe: audioPipe,
            micPipe: micPipe,
            enableSystemAudio: enableSystemAudio,
            enableMic: enableMic,
            audioSampleRate: max(audioSampleRate, 8_000),
            audioChannels: max(audioChannels, 1),
            excludeCurrentProcessAudio: excludeCurrentProcessAudio,
            micBackend: micBackend,
            selectedMicrophoneID: selectedMicrophoneID,
            micRetryIntervalSecs: max(micRetryIntervalSecs, 1),
            boostMicVolume: boostMicVolume,
            parentPID: parentPID,
            ffmpegPID: ffmpegPID
        )
    }
}
