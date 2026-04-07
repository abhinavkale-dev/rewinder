import AudioToolbox
import AVFoundation
import CoreAudio
import Foundation

final class MicrophoneVolumeManager {
    private var savedVolume: Float32?
    private var savedDeviceID: AudioDeviceID?

    func boost(selectedMicrophoneID: String?) {
        let deviceID = resolveInputDevice(selectedMicrophoneID: selectedMicrophoneID)
        guard deviceID != 0 else {
            fputs("phase: mic_volume_boost_skipped reason=no_input_device\n", stderr)
            fflush(stderr)
            return
        }
        guard hasVolumeControl(deviceID: deviceID) else {
            fputs("phase: mic_volume_boost_skipped reason=no_volume_control\n", stderr)
            fflush(stderr)
            return
        }
        do {
            let current = try getVolume(deviceID: deviceID)
            savedVolume = current
            savedDeviceID = deviceID

            if current >= 0.99 {
                fputs(
                    "phase: mic_volume_boost_skipped reason=already_max volume=\(String(format: "%.2f", current))\n",
                    stderr
                )
                fflush(stderr)
                return
            }

            try setVolume(deviceID: deviceID, volume: 1.0)
            fputs(
                "phase: mic_volume_boosted original=\(String(format: "%.2f", current)) new=1.00\n",
                stderr
            )
            fflush(stderr)
        } catch {
            fputs("phase: mic_volume_boost_failed error=\(error)\n", stderr)
            fflush(stderr)
        }
    }

    func restore() {
        guard let volume = savedVolume, let deviceID = savedDeviceID else {
            return
        }
        savedVolume = nil
        savedDeviceID = nil

        do {
            try setVolume(deviceID: deviceID, volume: volume)
            fputs(
                "phase: mic_volume_restored volume=\(String(format: "%.2f", volume))\n",
                stderr
            )
            fflush(stderr)
        } catch {
            fputs("phase: mic_volume_restore_failed error=\(error)\n", stderr)
            fflush(stderr)
        }
    }

    private func resolveInputDevice(selectedMicrophoneID: String?) -> AudioDeviceID {
        guard let uid = selectedMicrophoneID, !uid.isEmpty else {
            return defaultInputDevice()
        }

        var propSize: UInt32 = 0
        var devicesAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        guard AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject), &devicesAddress, 0, nil, &propSize
        ) == noErr else {
            return defaultInputDevice()
        }

        let count = Int(propSize) / MemoryLayout<AudioDeviceID>.size
        var deviceIDs = [AudioDeviceID](repeating: 0, count: count)
        guard AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject), &devicesAddress, 0, nil, &propSize, &deviceIDs
        ) == noErr else {
            return defaultInputDevice()
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
               let deviceUID = cfUID as String?, deviceUID == uid
            {
                return id
            }
        }

        return defaultInputDevice()
    }

    private func defaultInputDevice() -> AudioDeviceID {
        var deviceID = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultInputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject),
            &address,
            0,
            nil,
            &size,
            &deviceID
        )
        return deviceID
    }

    private func hasVolumeControl(deviceID: AudioDeviceID) -> Bool {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyVolumeScalar,
            mScope: kAudioDevicePropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        return AudioObjectHasProperty(deviceID, &address)
    }

    private func getVolume(deviceID: AudioDeviceID) throws -> Float32 {
        var volume: Float32 = 0
        var size = UInt32(MemoryLayout<Float32>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyVolumeScalar,
            mScope: kAudioDevicePropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        let status = AudioObjectGetPropertyData(deviceID, &address, 0, nil, &size, &volume)
        guard status == noErr else {
            throw VolumeManagerError.getVolumeFailed(status)
        }
        return volume
    }

    private func setVolume(deviceID: AudioDeviceID, volume: Float32) throws {
        var newVolume = volume
        let size = UInt32(MemoryLayout<Float32>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyVolumeScalar,
            mScope: kAudioDevicePropertyScopeInput,
            mElement: kAudioObjectPropertyElementMain
        )
        let status = AudioObjectSetPropertyData(deviceID, &address, 0, nil, size, &newVolume)
        guard status == noErr else {
            throw VolumeManagerError.setVolumeFailed(status)
        }
    }
}

enum VolumeManagerError: Error, CustomStringConvertible {
    case getVolumeFailed(OSStatus)
    case setVolumeFailed(OSStatus)

    var description: String {
        switch self {
        case let .getVolumeFailed(status):
            return "get_volume_failed(status=\(status))"
        case let .setVolumeFailed(status):
            return "set_volume_failed(status=\(status))"
        }
    }
}
