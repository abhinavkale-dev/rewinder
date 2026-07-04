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
