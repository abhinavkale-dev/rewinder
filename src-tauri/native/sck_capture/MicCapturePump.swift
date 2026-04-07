import AVFoundation
import CoreMedia
import Foundation

final class MicCapturePump: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate {
    private let sink: CaptureOutput
    private let session = AVCaptureSession()
    private let queue: DispatchQueue
    private let targetSampleRate: Int
    private let targetChannels: Int
    private let selectedMicrophoneID: String?
    private let onFailure: (String, String) -> Void
    private var isRunning = false
    private var micInput: AVCaptureDeviceInput?
    private var micOutput: AVCaptureAudioDataOutput?
    private var sessionRuntimeErrorObserver: NSObjectProtocol?
    private var deviceDisconnectedObserver: NSObjectProtocol?

    private(set) var activeMicrophoneID: String?
    private(set) var activeMicrophoneName: String?

    init(
        sink: CaptureOutput,
        targetSampleRate: Int,
        targetChannels: Int,
        queue: DispatchQueue,
        selectedMicrophoneID: String?,
        onFailure: @escaping (String, String) -> Void
    ) {
        self.sink = sink
        self.targetSampleRate = max(targetSampleRate, 8_000)
        self.targetChannels = max(targetChannels, 1)
        self.queue = queue
        self.selectedMicrophoneID = selectedMicrophoneID
        self.onFailure = onFailure
    }

    func start() throws {
        if isRunning {
            return
        }

        guard let micDevice = resolveMicrophoneDevice(selectedMicrophoneID) else {
            throw CaptureError.microphoneDeviceUnavailable
        }

        let micInput: AVCaptureDeviceInput
        do {
            micInput = try AVCaptureDeviceInput(device: micDevice)
        } catch {
            throw CaptureError.microphoneCaptureSetup("unable to create mic input: \(error)")
        }

        let micOutput = AVCaptureAudioDataOutput()
        micOutput.audioSettings = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: targetSampleRate,
            AVNumberOfChannelsKey: targetChannels,
            AVLinearPCMBitDepthKey: 32,
            AVLinearPCMIsFloatKey: true,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false
        ]
        micOutput.setSampleBufferDelegate(self, queue: queue)

        session.beginConfiguration()

        if session.canAddInput(micInput) {
            session.addInput(micInput)
        } else {
            session.commitConfiguration()
            throw CaptureError.microphoneCaptureSetup("cannot add mic input to session")
        }

        if session.canAddOutput(micOutput) {
            session.addOutput(micOutput)
        } else {
            session.commitConfiguration()
            throw CaptureError.microphoneCaptureSetup("cannot add mic output to session")
        }

        session.commitConfiguration()
        self.micInput = micInput
        self.micOutput = micOutput
        self.activeMicrophoneID = micDevice.uniqueID
        self.activeMicrophoneName = micDevice.localizedName
        registerObservers(for: micDevice)
        fputs("phase: mic_capture_configured\n", stderr)
        fflush(stderr)

        session.startRunning()
        if session.isRunning {
            isRunning = true
            fputs("phase: mic_capture_session_running\n", stderr)
            fflush(stderr)
        } else {
            throw CaptureError.microphoneCaptureSetup("microphone session failed to start")
        }
    }

    func stop() {
        unregisterObservers()
        if session.isRunning {
            session.stopRunning()
        }
        session.beginConfiguration()
        if let micOutput {
            session.removeOutput(micOutput)
        }
        if let micInput {
            session.removeInput(micInput)
        }
        session.commitConfiguration()
        isRunning = false
        micOutput = nil
        micInput = nil
        activeMicrophoneID = nil
        activeMicrophoneName = nil
    }

    func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        _ = output
        _ = connection
        sink.ingestMicrophoneSampleBuffer(sampleBuffer)
    }

    private func resolveMicrophoneDevice(_ selectedID: String?) -> AVCaptureDevice? {
        if let selectedID, !selectedID.isEmpty {
            if let found = AVCaptureDevice.devices(for: .audio).first(where: { $0.uniqueID == selectedID }) {
                return found
            }
            fputs("phase: mic_selected_device_not_found device_id=\(selectedID)\n", stderr)
            fflush(stderr)
            return AVCaptureDevice.default(for: .audio)
        }
        return AVCaptureDevice.default(for: .audio)
    }

    private func registerObservers(for device: AVCaptureDevice) {
        unregisterObservers()

        sessionRuntimeErrorObserver = NotificationCenter.default.addObserver(
            forName: .AVCaptureSessionRuntimeError,
            object: session,
            queue: nil
        ) { [weak self] notification in
            guard let self else {
                return
            }
            self.queue.async {
                let reason = (notification.userInfo?[AVCaptureSessionErrorKey] as? NSError)?
                    .localizedDescription ?? "session runtime error"
                self.handleFailure(code: "mic_runtime_detached", reason: reason)
            }
        }

        deviceDisconnectedObserver = NotificationCenter.default.addObserver(
            forName: .AVCaptureDeviceWasDisconnected,
            object: nil,
            queue: nil
        ) { [weak self] notification in
            guard let self else {
                return
            }
            self.queue.async {
                guard let disconnected = notification.object as? AVCaptureDevice else {
                    return
                }
                guard disconnected.uniqueID == device.uniqueID else {
                    return
                }
                self.handleFailure(code: "mic_device_missing", reason: "selected microphone disconnected")
            }
        }
    }

    private func unregisterObservers() {
        if let sessionRuntimeErrorObserver {
            NotificationCenter.default.removeObserver(sessionRuntimeErrorObserver)
            self.sessionRuntimeErrorObserver = nil
        }
        if let deviceDisconnectedObserver {
            NotificationCenter.default.removeObserver(deviceDisconnectedObserver)
            self.deviceDisconnectedObserver = nil
        }
    }

    private func handleFailure(code: String, reason: String) {
        stop()
        onFailure(code, reason)
    }
}
