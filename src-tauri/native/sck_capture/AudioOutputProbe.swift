import CoreAudio
import Foundation

enum AudioOutputProbe {
    private static let sourceInternalSpeaker: UInt32 = 0x6973_706B
    private static let sourceHeadphones: UInt32 = 0x6864_706E

    static func probe() -> (transport: String, source: String, echoProne: Bool) {
        guard let deviceID = defaultOutputDevice() else {
            return ("unknown", "unknown", true)
        }
        let transport = transportType(of: deviceID)
        let source = dataSource(of: deviceID)

        let echoProne: Bool
        switch transport {
        case kAudioDeviceTransportTypeBuiltIn:
            echoProne = source != sourceHeadphones
        case kAudioDeviceTransportTypeBluetooth, kAudioDeviceTransportTypeBluetoothLE:
            echoProne = false
        default:
            echoProne = true
        }
        return (name(forTransport: transport), name(forSource: source), echoProne)
    }

    static func defaultOutputDevice() -> AudioDeviceID? {
        var deviceID = AudioDeviceID(0)
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        let status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &deviceID
        )
        guard status == noErr, deviceID != 0 else {
            return nil
        }
        return deviceID
    }

    private static func transportType(of deviceID: AudioDeviceID) -> UInt32 {
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyTransportType,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        _ = AudioObjectGetPropertyData(deviceID, &address, 0, nil, &size, &value)
        return value
    }

    private static func dataSource(of deviceID: AudioDeviceID) -> UInt32 {
        var value: UInt32 = 0
        var size = UInt32(MemoryLayout<UInt32>.size)
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioDevicePropertyDataSource,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMain
        )
        _ = AudioObjectGetPropertyData(deviceID, &address, 0, nil, &size, &value)
        return value
    }

    private static func name(forTransport transport: UInt32) -> String {
        switch transport {
        case kAudioDeviceTransportTypeBuiltIn: return "builtin"
        case kAudioDeviceTransportTypeBluetooth: return "bluetooth"
        case kAudioDeviceTransportTypeBluetoothLE: return "bluetooth_le"
        case kAudioDeviceTransportTypeUSB: return "usb"
        case kAudioDeviceTransportTypeHDMI: return "hdmi"
        case kAudioDeviceTransportTypeDisplayPort: return "displayport"
        case kAudioDeviceTransportTypeAirPlay: return "airplay"
        case kAudioDeviceTransportTypeVirtual: return "virtual"
        case 0: return "unknown"
        default: return fourCC(transport)
        }
    }

    private static func name(forSource source: UInt32) -> String {
        switch source {
        case sourceInternalSpeaker: return "internal_speaker"
        case sourceHeadphones: return "headphones"
        case 0: return "none"
        default: return fourCC(source)
        }
    }

    private static func fourCC(_ value: UInt32) -> String {
        let bytes = [
            UInt8((value >> 24) & 0xFF),
            UInt8((value >> 16) & 0xFF),
            UInt8((value >> 8) & 0xFF),
            UInt8(value & 0xFF),
        ]
        let text = String(bytes: bytes, encoding: .ascii) ?? "????"
        return text.trimmingCharacters(in: .whitespaces)
    }
}
