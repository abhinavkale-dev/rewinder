import Foundation

struct FFIEnvelope<T: Decodable>: Decodable {
    let ok: Bool
    let data: T?
    let error: String?
}

struct Settings: Codable, Sendable, Equatable {
    var replayDurationSecs: Int
    var bufferDurationSecs: Int
    var fps: Int
    var videoResolution: Int
    var videoBitrateKbps: Int
    var audioBitrateKbps: Int
    var outputDir: String
    var hotkey: String
    var fallbackHotkeys: [String]
    var replayEnabled: Bool
    var audioMode: String
    var micEnabled: Bool
    var audioSampleRateHz: Int
    var audioChannels: Int
    var segmentTimeMs: Int
    var warmupDeferTtlMs: Int
    var qualityPolicy: String
    var qualityPreference: String
    var audioFallbackPolicy: String
    var micCaptureBackend: String
    var selectedMicrophoneId: String?
    var micFailurePolicy: String
    var micStartupTimeoutMs: Int
    var micRetryIntervalSecs: Int
    var micMixGainDb: Double
    var micAutoRequestPermission: Bool
    var micNoiseSuppression: Bool
    var audioStartupTimeoutMs: Int
    var profileRecoverHoldSecs: Int
    var excludeCurrentProcessAudio: Bool
    var savePathMode: String
    var audioSaveMode: String
    var performanceGuardEnabled: Bool
    var performanceGuardLevel: String
    var batteryGuardEnabled: Bool
    var batteryMaxFps: Int
    var systemVolumePercent: Int
    var selectedDisplayId: String?
}

struct PermissionState: Decodable, Sendable, Equatable {
    let screenRecordingGranted: Bool
    let systemAudioGranted: Bool
    let outputDirWritable: Bool
    let outputDirPermissionError: String?
    let reason: String?
}

struct EngineState: Decodable, Sendable {
    let lifecycleState: String
    let captureHealth: String
    let captureStartPhase: String?
    let audioHealth: String
    let saveStage: String
    let systemAudioPathReady: Bool
    let micPermissionStatus: String
    let micAttachState: String
    let hotkeyStatus: String
    var effectiveVideoResolution: Int
    var effectiveFps: Int
    let guardState: String
    let guardPrimaryReasonCode: String?
    let requestedVideoBitrateKbps: Int
    let effectiveVideoBitrateKbps: Int
    let degradeReason: String?
    let powerSource: String?
    let isArmed: Bool
    let isSaving: Bool
    let armBlocker: String?
    let armBlockerCode: String?
    let pendingSave: Bool
    let bufferFillSecs: Double
    let replayFillSecs: Double
    let replayTargetSecs: Double
    let captureRestartCount: Int
    let lastError: String?
    let permission: PermissionState
    var settings: Settings
}

struct GuardTransition: Decodable, Sendable {
    let action: String
    let guardState: String
    let hard: Bool
    let primaryReasonCode: String?
    let fromProfile: String?
    let toProfile: String?
    let sampledAtEpochMs: Double
}

struct ClipMetadata: Decodable, Sendable, Identifiable {
    let id: String
    let path: String
    let createdAtEpochMs: Double
    let durationSecs: Double
    let sizeBytes: Int
}

struct MicrophoneDevice: Decodable, Sendable, Identifiable {
    let id: String
    let name: String
    let isDefault: Bool
    let isAvailable: Bool
}

struct SaveReplayResult: Decodable, Sendable {
    let ok: Bool
    let queued: Bool
    let message: String?
    let error: String?
}

struct GrantResult: Decodable, Sendable {
    let permission: PermissionState
    let openedSettings: Bool
    let message: String
}
