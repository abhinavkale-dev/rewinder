import CoreMedia
import Dispatch
import Foundation
import ScreenCaptureKit

@MainActor
final class CaptureController {
    private static let shutdownTimeoutMs: Int = 1_500

    private let config: CaptureConfig
    private let pressureQueue = DispatchQueue(label: "rewinder.system.pressure", qos: .utility)
    private var stream: SCStream?
    private var output: CaptureOutput?
    private var micPump: MicCapturePump?
    private var micRetryTimer: DispatchSourceTimer?
    private var micRetryAttempt = 0
    private var volumeManager: MicrophoneVolumeManager?
    private var shutdownInFlight = false
    private var shutdownTimeoutTimer: DispatchSourceTimer?
    private var memoryPressureSource: DispatchSourceMemoryPressure?
    private var thermalStateObserver: NSObjectProtocol?
    private var lastSystemMemoryPressureLevel = "normal"
    private var lastThermalState = "nominal"

    init(config: CaptureConfig) {
        self.config = config
    }

    func start() async throws {
        let shareable = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard config.displayIndex < shareable.displays.count else {
            throw CaptureError.noDisplay(config.displayIndex)
        }

        let videoWriter = try PipeWriter(path: config.videoPipe)
        let display = shareable.displays[config.displayIndex]
        let filter = SCContentFilter(display: display, excludingWindows: [])

        let streamConfig = SCStreamConfiguration()
        streamConfig.width = config.width
        streamConfig.height = config.height
        streamConfig.pixelFormat = kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange
        streamConfig.minimumFrameInterval = CMTime(value: 1, timescale: CMTimeScale(config.fps))
        streamConfig.showsCursor = true
        streamConfig.capturesAudio = config.enableSystemAudio
        streamConfig.excludesCurrentProcessAudio = config.excludeCurrentProcessAudio
        streamConfig.sampleRate = config.audioSampleRate
        streamConfig.channelCount = config.audioChannels
        streamConfig.queueDepth = 3

        let useSckMicBackend = resolvedUsesNativeMicBackend()
        if #available(macOS 15.0, *) {
            streamConfig.captureMicrophone = useSckMicBackend
            if useSckMicBackend, let selectedMicrophoneID = config.selectedMicrophoneID {
                streamConfig.microphoneCaptureDeviceID = selectedMicrophoneID
            }
        } else if useSckMicBackend {
            throw CaptureError.unsupportedMicrophoneOutput
        }

        let output = CaptureOutput(
            videoWriter: videoWriter,
            targetFps: config.fps,
            requestShutdown: { [weak self] cause, exitCode in
                Task { @MainActor in
                    self?.requestShutdown(cause: cause, exitCode: exitCode)
                }
            },
            systemAudioPipePath: config.audioPipe,
            micPipePath: config.micPipe,
            targetAudioSampleRate: config.audioSampleRate,
            targetAudioChannels: config.audioChannels
        )
        let stream = SCStream(filter: filter, configuration: streamConfig, delegate: output)

        let videoQueue = DispatchQueue(label: "rewinder.sck.video", qos: .userInitiated)
        let systemAudioQueue = DispatchQueue(
            label: "rewinder.sck.audio.system",
            qos: .userInteractive
        )
        let microphoneQueue = DispatchQueue(
            label: "rewinder.sck.audio.microphone",
            qos: .userInteractive
        )

        try stream.addStreamOutput(output, type: .screen, sampleHandlerQueue: videoQueue)
        if config.enableSystemAudio {
            try stream.addStreamOutput(output, type: .audio, sampleHandlerQueue: systemAudioQueue)
        }
        if useSckMicBackend {
            if #available(macOS 15.0, *) {
                emitMicBackendAttempt(backend: "sck_native", deviceID: config.selectedMicrophoneID)
                try stream.addStreamOutput(
                    output,
                    type: .microphone,
                    sampleHandlerQueue: microphoneQueue
                )
                emitMicBackendReady(backend: "sck_native", deviceID: config.selectedMicrophoneID)
            } else {
                throw CaptureError.unsupportedMicrophoneOutput
            }
        }

        self.stream = stream
        self.output = output

        fputs("stream start requested\n", stderr)
        fflush(stderr)
        try await stream.startCapture()
        fputs("stream started\n", stderr)
        fflush(stderr)

        output.markStreamStarted()
        startSystemPressureObservers()
        output.startCaptureInactivityWatchdog(on: systemAudioQueue)
        output.startSystemAudioPipeReconnectLoop()
        if config.enableMic {
            output.startMicPipeReconnectLoop()
        }
        let audioSampleRate = config.audioSampleRate
        let audioChannels = config.audioChannels
        systemAudioQueue.async {
            output.primeSystemAudioPipe(
                sampleRate: audioSampleRate,
                channels: audioChannels
            )
        }
        if config.enableMic {
            microphoneQueue.async {
                output.primeMicPipe(
                    sampleRate: audioSampleRate,
                    channels: audioChannels
                )
            }
            output.startMicSilenceFillerLoop(on: microphoneQueue)
        }

        if config.enableMic && config.boostMicVolume {
            let mgr = MicrophoneVolumeManager()
            mgr.boost(selectedMicrophoneID: config.selectedMicrophoneID)
            self.volumeManager = mgr
        }

        if config.enableMic && !useSckMicBackend {
            startMicPump(on: microphoneQueue, announceRecovered: false)
        }

        fputs(
            "ScreenCaptureKit started: display=\(config.displayIndex) size=\(config.width)x\(config.height) fps=\(config.fps) system_audio=\(config.enableSystemAudio) mic=\(config.enableMic) mic_backend=\(config.micBackend) sample_rate=\(config.audioSampleRate) channels=\(config.audioChannels) excludes_self_audio=\(config.excludeCurrentProcessAudio)\n",
            stderr
        )
        fflush(stderr)
    }

    func stopAndExit(_ code: Int32) {
        requestShutdown(cause: "signal", exitCode: code)
    }

    private func requestShutdown(cause: String, exitCode: Int32) {
        guard !shutdownInFlight else {
            return
        }
        shutdownInFlight = true
        fputs("phase: helper_shutdown_requested cause=\(cause) exit_code=\(exitCode)\n", stderr)
        fflush(stderr)

        stopSystemPressureObservers()
        volumeManager?.restore()
        volumeManager = nil
        output?.beginShutdown()
        stopMicRetryTimer()
        micPump?.stop()
        micPump = nil
        armShutdownTimeout(cause: cause, exitCode: exitCode)

        Task { @MainActor [weak self] in
            guard let self else {
                return
            }
            if let stream = self.stream {
                do {
                    try await stream.stopCapture()
                } catch {
                    let reason = String(describing: error).replacingOccurrences(of: "\n", with: " ")
                    fputs("phase: helper_stream_stop_failed cause=\(cause) error=\(reason)\n", stderr)
                    fflush(stderr)
                }
            }
            self.finishShutdown(cause: cause, exitCode: exitCode)
        }
    }

    private func armShutdownTimeout(cause: String, exitCode: Int32) {
        shutdownTimeoutTimer?.setEventHandler {}
        shutdownTimeoutTimer?.cancel()
        let timer = DispatchSource.makeTimerSource(queue: DispatchQueue.main)
        timer.schedule(deadline: .now() + .milliseconds(Self.shutdownTimeoutMs))
        timer.setEventHandler { [weak self] in
            guard let self, self.shutdownInFlight else {
                return
            }
            fputs("phase: helper_shutdown_timeout cause=\(cause) exit_code=\(exitCode)\n", stderr)
            fflush(stderr)
            self.finishShutdown(cause: cause, exitCode: exitCode)
        }
        shutdownTimeoutTimer = timer
        timer.resume()
    }

    private func finishShutdown(cause: String, exitCode: Int32) {
        guard shutdownInFlight else {
            return
        }
        shutdownInFlight = false
        shutdownTimeoutTimer?.setEventHandler {}
        shutdownTimeoutTimer?.cancel()
        shutdownTimeoutTimer = nil

        if let stream, let output {
            try? stream.removeStreamOutput(output, type: .screen)
            if config.enableSystemAudio {
                try? stream.removeStreamOutput(output, type: .audio)
            }
            if config.enableMic && resolvedUsesNativeMicBackend() {
                if #available(macOS 15.0, *) {
                    try? stream.removeStreamOutput(output, type: .microphone)
                }
            }
        }

        output?.closeWriters()
        output = nil
        stream = nil
        stopMicRetryTimer()
        micPump?.stop()
        micPump = nil
        stopSystemPressureObservers()

        fputs("phase: helper_shutdown_complete cause=\(cause)\n", stderr)
        fflush(stderr)
        exit(exitCode)
    }

    private func resolvedUsesNativeMicBackend() -> Bool {
        guard config.enableMic else {
            return false
        }
        switch config.micBackend {
        case "sck_native":
            return true
        case "auto":
            if #available(macOS 15.0, *) {
                return true
            }
            return false
        default:
            return false
        }
    }

    private func emitMicBackendAttempt(backend: String, deviceID: String?) {
        let resolvedDeviceID = deviceID ?? "default"
        let deviceName = resolveMicrophoneName(for: deviceID) ?? "system_default"
        fputs(
            "phase: mic_backend_attempt backend=\(backend) device_id=\(resolvedDeviceID) device_name=\(sanitizeLogValue(deviceName))\n",
            stderr
        )
        fflush(stderr)
    }

    private func emitMicBackendReady(backend: String, deviceID: String?) {
        let resolvedDeviceID = deviceID ?? "default"
        let deviceName = resolveMicrophoneName(for: deviceID) ?? "system_default"
        fputs(
            "phase: mic_backend_ready backend=\(backend) device_id=\(resolvedDeviceID) device_name=\(sanitizeLogValue(deviceName))\n",
            stderr
        )
        fflush(stderr)
    }

    private func emitMicBackendError(backend: String, code: String, reason: String) {
        let sanitized = reason.replacingOccurrences(of: "\n", with: " ")
        fputs("phase: mic_backend_error backend=\(backend) code=\(code) reason=\(sanitized)\n", stderr)
        fflush(stderr)
    }

    private func emitMicBackendRecovered(backend: String, deviceID: String?) {
        let resolvedDeviceID = deviceID ?? "default"
        let deviceName = resolveMicrophoneName(for: deviceID) ?? "system_default"
        fputs(
            "phase: mic_backend_recovered backend=\(backend) device_id=\(resolvedDeviceID) device_name=\(sanitizeLogValue(deviceName))\n",
            stderr
        )
        fflush(stderr)
    }

    private func startMicPump(on audioQueue: DispatchQueue, announceRecovered: Bool) {
        guard config.enableMic else {
            return
        }
        let selectedMicrophoneID = config.selectedMicrophoneID
        emitMicBackendAttempt(backend: "avcapture", deviceID: selectedMicrophoneID)

        let micPump = MicCapturePump(
            sink: output!,
            targetSampleRate: config.audioSampleRate,
            targetChannels: config.audioChannels,
            queue: audioQueue,
            selectedMicrophoneID: selectedMicrophoneID
        ) { [weak self] code, reason in
            Task { @MainActor [weak self] in
                self?.handleMicPumpFailure(code: code, reason: reason, queue: audioQueue)
            }
        }
        do {
            try micPump.start()
            self.micPump = micPump
            stopMicRetryTimer()
            micRetryAttempt = 0
            fputs("microphone capture started via AVCaptureSession backend\n", stderr)
            fflush(stderr)
            if announceRecovered {
                emitMicBackendRecovered(
                    backend: "avcapture",
                    deviceID: micPump.activeMicrophoneID ?? selectedMicrophoneID
                )
            } else {
                emitMicBackendReady(
                    backend: "avcapture",
                    deviceID: micPump.activeMicrophoneID ?? selectedMicrophoneID
                )
            }
        } catch {
            self.micPump = nil
            let reason = String(describing: error)
            emitMicBackendError(backend: "avcapture", code: "mic_backend_setup_failed", reason: reason)
            scheduleMicRetry(queue: audioQueue, reasonCode: "mic_backend_setup_failed")
            // Best effort: keep replay alive with system audio when mic path fails.
            fputs("microphone capture unavailable (best effort): \(error)\n", stderr)
            fflush(stderr)
        }
    }

    private func handleMicPumpFailure(code: String, reason: String, queue: DispatchQueue) {
        micPump = nil
        emitMicBackendError(backend: "avcapture", code: code, reason: reason)
        scheduleMicRetry(queue: queue, reasonCode: code)
    }

    private func scheduleMicRetry(queue: DispatchQueue, reasonCode: String) {
        guard config.enableMic, !shutdownInFlight else {
            return
        }
        stopMicRetryTimer()
        micRetryAttempt += 1
        let baseDelay = max(config.micRetryIntervalSecs, 1)
        let delaySecs = min(baseDelay * max(micRetryAttempt, 1), 30)
        fputs("phase: mic_backend_retry_scheduled backend=avcapture reason=\(reasonCode) delay_ms=\(delaySecs * 1000)\n", stderr)
        fflush(stderr)
        let timer = DispatchSource.makeTimerSource(queue: DispatchQueue.main)
        timer.schedule(deadline: .now() + .seconds(delaySecs))
        timer.setEventHandler { [weak self] in
            guard let self, !self.shutdownInFlight else {
                return
            }
            self.micRetryTimer = nil
            self.startMicPump(on: queue, announceRecovered: true)
        }
        micRetryTimer = timer
        timer.resume()
    }

    private func stopMicRetryTimer() {
        micRetryTimer?.setEventHandler {}
        micRetryTimer?.cancel()
        micRetryTimer = nil
    }

    private func startSystemPressureObservers() {
        emitSystemMemoryPressure("normal", force: true)
        emitThermalState(currentThermalStateLabel(), force: true)

        let source = DispatchSource.makeMemoryPressureSource(
            eventMask: [.normal, .warning, .critical],
            queue: pressureQueue
        )
        source.setEventHandler { [weak self, weak source] in
            guard let source else {
                return
            }
            let level: String
            if source.data.contains(.critical) {
                level = "critical"
            } else if source.data.contains(.warning) {
                level = "warning"
            } else {
                level = "normal"
            }
            Task { @MainActor [weak self] in
                self?.emitSystemMemoryPressure(level)
            }
        }
        memoryPressureSource = source
        source.resume()

        thermalStateObserver = NotificationCenter.default.addObserver(
            forName: ProcessInfo.thermalStateDidChangeNotification,
            object: nil,
            queue: nil
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                guard let self else {
                    return
                }
                self.emitThermalState(self.currentThermalStateLabel())
            }
        }
    }

    private func stopSystemPressureObservers() {
        memoryPressureSource?.setEventHandler {}
        memoryPressureSource?.cancel()
        memoryPressureSource = nil

        if let observer = thermalStateObserver {
            NotificationCenter.default.removeObserver(observer)
            thermalStateObserver = nil
        }
    }

    private func emitSystemMemoryPressure(_ level: String, force: Bool = false) {
        guard force || level != lastSystemMemoryPressureLevel else {
            return
        }
        lastSystemMemoryPressureLevel = level
        fputs("system_memory_pressure=\(level)\n", stderr)
        fflush(stderr)
    }

    private func emitThermalState(_ level: String, force: Bool = false) {
        guard force || level != lastThermalState else {
            return
        }
        lastThermalState = level
        fputs("thermal_state=\(level)\n", stderr)
        fflush(stderr)
    }

    private func currentThermalStateLabel() -> String {
        switch ProcessInfo.processInfo.thermalState {
        case .nominal:
            return "nominal"
        case .fair:
            return "fair"
        case .serious:
            return "serious"
        case .critical:
            return "critical"
        @unknown default:
            return "nominal"
        }
    }

    private func resolveMicrophoneName(for deviceID: String?) -> String? {
        let devices = AVCaptureDevice.devices(for: .audio)
        if let deviceID, !deviceID.isEmpty {
            return devices.first(where: { $0.uniqueID == deviceID })?.localizedName
        }
        return AVCaptureDevice.default(for: .audio)?.localizedName
    }

    private func sanitizeLogValue(_ value: String) -> String {
        value
            .replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
