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
    }

    private func resolveInputDeviceID(for uid: String) -> AudioDeviceID? {
        var propSize: UInt32 = 0
        var devicesAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        guard AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject), &devicesAddress, 0, nil, &propSize
        ) == noErr else {
            return nil
        }

        let count = Int(propSize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: count)
        guard AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject), &devicesAddress, 0, nil, &propSize, &deviceIDs
        ) == noErr else {
            return nil
        }

        for id in deviceIDs {
            var uidAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyDeviceUID,
                mScope: kAudioObjectPropertyScopeGlobal,
                mElement: kAudioObjectPropertyElementMain
            )
            var cfUID: CFString?
            var uidSize = UInt32(MemoryLayout<CFString?>.size)
            if AudioObjectGetPropertyData(id, &uidAddress, 0, nil, &uidSize, &cfUID) == noErr,
               let deviceUID = cfUID as String?, deviceUID == uid {
                return id
            }
        }

        return nil
    }

    private func handleTap(buffer: AVAudioPCMBuffer, when: AVAudioTime) {
        guard let sampleBuffer = makeSampleBuffer(from: buffer, when: when) else {
            return
        }
        sink.ingestMicrophoneSampleBuffer(sampleBuffer)
    }

    private func makeSampleBuffer(
        from pcmBuffer: AVAudioPCMBuffer,
        when: AVAudioTime
    ) -> CMSampleBuffer? {
        let frameCount = CMItemCount(pcmBuffer.frameLength)
        guard frameCount > 0 else {
            return nil
        }

        var asbd = pcmBuffer.format.streamDescription.pointee
        var formatDescription: CMAudioFormatDescription?
        let formatStatus = CMAudioFormatDescriptionCreate(
            allocator: kCFAllocatorDefault,
            asbd: &asbd,
            layoutSize: 0,
            layout: nil,
            magicCookieSize: 0,
            magicCookie: nil,
            extensions: nil,
            formatDescriptionOut: &formatDescription
        )
        guard formatStatus == noErr, let formatDescription else {
            return nil
        }

        let sampleRate = asbd.mSampleRate > 0 ? asbd.mSampleRate : 48_000
        let pts = CMTime(value: when.sampleTime, timescale: CMTimeScale(sampleRate))
        var timing = CMSampleTimingInfo(
            duration: CMTime(value: 1, timescale: CMTimeScale(sampleRate)),
            presentationTimeStamp: pts,
            decodeTimeStamp: .invalid
        )

        var sampleBuffer: CMSampleBuffer?
        let createStatus = CMSampleBufferCreate(
            allocator: kCFAllocatorDefault,
            dataBuffer: nil,
            dataReady: false,
            makeDataReadyCallback: nil,
            refcon: nil,
            formatDescription: formatDescription,
            sampleCount: frameCount,
            sampleTimingEntryCount: 1,
            sampleTimingArray: &timing,
            sampleSizeEntryCount: 0,
            sampleSizeArray: nil,
            sampleBufferOut: &sampleBuffer
        )
        guard createStatus == noErr, let sampleBuffer else {
            return nil
        }

        let attachStatus = CMSampleBufferSetDataBufferFromAudioBufferList(
            sampleBuffer,
            blockBufferAllocator: kCFAllocatorDefault,
            blockBufferMemoryAllocator: kCFAllocatorDefault,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            bufferList: pcmBuffer.audioBufferList
        )
        guard attachStatus == noErr else {
            return nil
        }

        return sampleBuffer
    }

    private func registerConfigChangeObserver() {
        unregisterConfigChangeObserver()
        configChangeObserver = NotificationCenter.default.addObserver(
            forName: .AVAudioEngineConfigurationChange,
            object: engine,
            queue: nil
        ) { [weak self] _ in
            guard let self else {
                return
            }
            self.queue.async {
                self.handleFailure(
                    code: "mic_voice_processing_config_changed",
                    reason: "audio engine configuration changed"
                )
            }
        }
    }

    private func unregisterConfigChangeObserver() {
        if let configChangeObserver {
            NotificationCenter.default.removeObserver(configChangeObserver)
            self.configChangeObserver = nil
        }
    }

    private func handleFailure(code: String, reason: String) {
        stop()
        onFailure(code, reason)
    }
}
