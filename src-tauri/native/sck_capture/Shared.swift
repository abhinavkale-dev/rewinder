import AudioToolbox
import AVFoundation
import CoreGraphics
import CoreMedia
import CoreVideo
import Darwin
import Foundation
import ScreenCaptureKit

let streamInterruptedExitCode: Int32 = 73

extension UInt64 {
    func saturatingAdding(_ value: UInt64) -> UInt64 {
        addingReportingOverflow(value).overflow ? UInt64.max : self + value
    }
}

struct CaptureConfig {
    let width: Int
    let height: Int
    let fps: Int
    let displayIndex: Int
    let displayID: CGDirectDisplayID?
    let videoPipe: String
    let audioPipe: String?
    let micPipe: String?
    let enableSystemAudio: Bool
    let enableMic: Bool
    let audioSampleRate: Int
    let audioChannels: Int
    let excludeCurrentProcessAudio: Bool
    let micBackend: String
    let selectedMicrophoneID: String?
    let micRetryIntervalSecs: Int
    let boostMicVolume: Bool
    let watchAudioRoute: Bool
    let parentPID: pid_t?
    let ffmpegPID: pid_t?
}

enum CaptureError: Error, CustomStringConvertible {
    case invalidArgs(String)
    case noDisplay(Int)
    case missingPipe(String)
    case pipeOpenFailed(String, Int32)
    case pipeOpenTimeout(String, Int32)
    case unsupportedMicrophoneOutput
    case microphoneDeviceUnavailable
    case microphoneCaptureSetup(String)

    var description: String {
        switch self {
        case let .invalidArgs(message):
            return "invalid args: \(message)"
        case let .noDisplay(index):
            return "display index out of range: \(index)"
        case let .missingPipe(path):
            return "missing pipe: \(path)"
        case let .pipeOpenFailed(path, code):
            return "failed to open pipe \(path): errno=\(code) (\(String(cString: strerror(code))))"
        case let .pipeOpenTimeout(path, code):
            return "timed out opening pipe \(path): errno=\(code) (\(String(cString: strerror(code))))"
        case .unsupportedMicrophoneOutput:
            return "microphone capture requires macOS 15+"
        case .microphoneDeviceUnavailable:
            return "no microphone device available"
        case let .microphoneCaptureSetup(reason):
            return "failed to setup microphone capture: \(reason)"
        }
    }
}
