import AudioToolbox
import AVFoundation
import CoreAudio
import CoreMedia
import Foundation

final class VoiceProcessingMicPump {
    private let sink: CaptureOutput
    private let queue: DispatchQueue
    private let selectedMicrophoneID: String?
    private let onFailure: (String, String) -> Void

    private let engine = AVAudioEngine()
    private var isRunning = false
    private var configChangeObserver: NSObjectProtocol?

    private(set) var activeMicrophoneID: String?
    private(set) var activeMicrophoneName: String?

    init(
        sink: CaptureOutput,
        queue: DispatchQueue,
        selectedMicrophoneID: String?,
        onFailure: @escaping (String, String) -> Void
    ) {
        self.sink = sink
        self.queue = queue
        self.selectedMicrophoneID = selectedMicrophoneID
        self.onFailure = onFailure
    }

    func start() throws {
        if isRunning {
            return
        }

        let input = engine.inputNode
        do {
            try input.setVoiceProcessingEnabled(true)
        } catch {
            throw CaptureError.microphoneCaptureSetup(
                "unable to enable voice processing: \(error)"
            )
        }

        var boundSelectedDevice = false
        if let selectedMicrophoneID, !selectedMicrophoneID.isEmpty {
            if let deviceID = resolveInputDeviceID(for: selectedMicrophoneID),
               let audioUnit = input.audioUnit {
                var target = deviceID
                let status = AudioUnitSetProperty(
                    audioUnit,
                    kAudioOutputUnitProperty_CurrentDevice,
                    kAudioUnitScope_Global,
                    0,
                    &target,
                    UInt32(MemoryLayout<AudioDeviceID>.size)
                )
                if status == noErr {
                    boundSelectedDevice = true
                    fputs(
                        "phase: mic_voice_processing_device_selected device_id=\(selectedMicrophoneID)\n",
                        stderr
                    )
                } else {
                    fputs(
                        "phase: mic_voice_processing_device_select_failed device_id=\(selectedMicrophoneID) status=\(status)\n",
                        stderr
                    )
                }
            } else {
                fputs(
                    "phase: mic_voice_processing_device_not_found device_id=\(selectedMicrophoneID)\n",
                    stderr
                )
            }
            fflush(stderr)
        }

        if #available(macOS 14.0, *) {
            input.voiceProcessingOtherAudioDuckingConfiguration =
                AVAudioVoiceProcessingOtherAudioDuckingConfiguration(
                    enableAdvancedDucking: false,
                    duckingLevel: .min
                )
        }

        if boundSelectedDevice, let selectedMicrophoneID,
           let device = AVCaptureDevice.devices(for: .audio)
               .first(where: { $0.uniqueID == selectedMicrophoneID }) {
            activeMicrophoneID = device.uniqueID
            activeMicrophoneName = device.localizedName
        } else {
            activeMicrophoneID = AVCaptureDevice.default(for: .audio)?.uniqueID
            activeMicrophoneName = AVCaptureDevice.default(for: .audio)?.localizedName
        }

        let tapFormat = input.outputFormat(forBus: 0)
        guard tapFormat.sampleRate > 0, tapFormat.channelCount > 0 else {
            throw CaptureError.microphoneCaptureSetup(
                "voice processing returned an invalid input format"
            )
        }
        fputs(
            "phase: mic_voice_processing_format sample_rate=\(Int(tapFormat.sampleRate)) channels=\(tapFormat.channelCount)\n",
            stderr
        )
        fflush(stderr)

        input.installTap(onBus: 0, bufferSize: 1024, format: tapFormat) { [weak self] buffer, when in
            guard let self else {
                return
            }
            self.handleTap(buffer: buffer, when: when)
        }

        registerConfigChangeObserver()

        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            unregisterConfigChangeObserver()
            throw CaptureError.microphoneCaptureSetup(
                "voice processing engine failed to start: \(error)"
            )
        }

        isRunning = true
        fputs("microphone capture started via AVAudioEngine voice-processing backend\n", stderr)
        fflush(stderr)
    }

    func stop() {
        unregisterConfigChangeObserver()
        if isRunning {
            engine.inputNode.removeTap(onBus: 0)
            engine.stop()
            try? engine.inputNode.setVoiceProcessingEnabled(false)
        }
        isRunning = false
        activeMicrophoneID = nil
        activeMicrophoneName = nil
