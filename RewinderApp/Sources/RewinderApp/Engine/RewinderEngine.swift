import AppKit
import Foundation
import Observation
import CRewinderFFI

private let eventTrampoline: @convention(c) (
    UnsafePointer<CChar>?, UnsafePointer<CChar>?, UnsafeMutableRawPointer?
) -> Void = { eventPtr, jsonPtr, ctx in
    guard let ctx, let eventPtr, let jsonPtr else { return }
    let event = String(cString: eventPtr)
    let json = String(cString: jsonPtr)
    let engine = Unmanaged<RewinderEngine>.fromOpaque(ctx).takeUnretainedValue()
    Task { @MainActor in
        engine.handleEvent(name: event, json: json)
    }
}

private struct EngineHandle: @unchecked Sendable {
    let raw: OpaquePointer
}

@MainActor
@Observable
final class RewinderEngine {
    private(set) var engineState: EngineState?
    private(set) var settings: Settings?
    private(set) var clips: [ClipMetadata] = []
    private(set) var microphones: [MicrophoneDevice] = []

    private(set) var statusLine: String = "Starting up…"
    private(set) var bootError: String?
    private(set) var lastEvent: String = "—"
    var isSubmittingSettings: Bool = false

    var pendingNavigation: AppView? = nil

    private(set) var saveConfirmations: Int = 0

    private(set) var lastGuardTransition: GuardTransition?

    @ObservationIgnored private var confirmedClipPaths: [String] = []

    @ObservationIgnored var onStateChange: (@MainActor () -> Void)?

    @ObservationIgnored var onClipSaved: (@MainActor () -> Void)?

    @ObservationIgnored nonisolated(unsafe) private var handle: OpaquePointer?
    @ObservationIgnored private var permissionPollTask: Task<Void, Never>?

    @ObservationIgnored private var pendingPatch: [String: Any] = [:]
    @ObservationIgnored private var patchFlushTask: Task<Void, Never>?
    @ObservationIgnored private var patchGeneration = 0
    @ObservationIgnored private var lastPatchFlush: Date = .distantPast
    @ObservationIgnored private let patchLeadingDelayMs = 60
    @ObservationIgnored private let patchCoalesceWindow: TimeInterval = 0.25

    @ObservationIgnored private let commandQueue =
        DispatchQueue(label: "com.rewinder.engine.commands", qos: .userInitiated)

    private let decoder = JSONDecoder()
    private let encoder = JSONEncoder()

    init() {
        BundleResources.configureEnvironment()
        handle = rewinder_init()
        guard let handle else {
            bootError = "Engine failed to initialize"
            statusLine = bootError!
            return
        }
        if !UserDefaults.standard.bool(forKey: "hasCompletedOnboarding") {
            if let result = rewinder_set_replay_enabled(handle, false) {
                rewinder_free_string(result)
            }
        }
        let ctx = Unmanaged.passUnretained(self).toOpaque()
        rewinder_set_event_callback(handle, eventTrampoline, ctx)
        refreshState()
        refreshSettings()
    }

    deinit {
        permissionPollTask?.cancel()
        if let handle {
            rewinder_shutdown(handle)
        }
    }

    func refreshState() {
        guard let handle else { return }
        applyStateEnvelope(rewinder_get_engine_state(handle))
    }

    func refreshStateAsync() {
        guard handle != nil else { return }
        offload({ rewinder_get_engine_state($0) }) { [weak self] text in
            self?.applyStateEnvelopeText(text)
        }
    }

    func refreshSettings() {
        guard let handle else { return }
        if let s: Settings = decodeEnvelope(rewinder_get_settings(handle)) {
            settings = s
        }
    }

    func defaultSettings() -> Settings? {
        decodeEnvelope(rewinder_default_settings(), label: "defaults")
    }

    func refreshClips() {
        guard let dir = settings?.outputDir, !dir.isEmpty else { return }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let scanned = Self.scanClips(inDirectory: dir)
            Task { @MainActor in
                self?.clips = scanned
            }
        }
    }

    nonisolated private static func scanClips(inDirectory dir: String) -> [ClipMetadata] {
        let keys: Set<URLResourceKey> = [.creationDateKey, .fileSizeKey, .isRegularFileKey]
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: URL(fileURLWithPath: dir),
            includingPropertiesForKeys: Array(keys),
            options: [.skipsHiddenFiles]
        ) else { return [] }

        return entries
            .filter { $0.pathExtension.lowercased() == "mp4" }
            .compactMap { file -> ClipMetadata? in
                guard let values = try? file.resourceValues(forKeys: keys),
                      values.isRegularFile == true else { return nil }
                let created = values.creationDate ?? .distantPast
                return ClipMetadata(
                    id: file.lastPathComponent,
                    path: file.path,
                    createdAtEpochMs: created.timeIntervalSince1970 * 1000,
                    durationSecs: 0,
                    sizeBytes: values.fileSize ?? 0
                )
            }
            .sorted { $0.createdAtEpochMs > $1.createdAtEpochMs }
    }

    func refreshMicrophones() {
        commandQueue.async { [weak self] in
            let ptr = rewinder_list_microphones()
            let text = ptr.map { p -> String in
                defer { rewinder_free_string(p) }
                return String(cString: p)
            }
            Task { @MainActor [weak self] in
                guard let self else { return }
                if let list: [MicrophoneDevice] = self.decodeEnvelopeText(text, label: "microphones") {
                    self.microphones = list
                }
            }
        }
    }

    func setReplayEnabled(_ enabled: Bool) {
        guard handle != nil else { return }
        if var state = engineState {
            state.settings.replayEnabled = enabled
            engineState = state
            settings = state.settings
        }
        offload({ rewinder_set_replay_enabled($0, enabled) }) { [weak self] text in
            self?.applyStateEnvelopeText(text)
        }
    }

    func resumeCapture() {
        guard handle != nil else { return }
        offload({ rewinder_resume_capture($0) }) { [weak self] text in
            self?.applyStateEnvelopeText(text)
        }
    }

    func saveReplay(hotkey: Bool = false) {
        guard let handle else { return }
        let result: SaveReplayResult?
        if hotkey {
            result = "\"hotkey\"".withCString {
                decodeEnvelope(rewinder_trigger_save_replay(handle, $0), label: "save")
            }
        } else {
            result = decodeEnvelope(rewinder_trigger_save_replay(handle, nil), label: "save")
        }
        if let result {
            statusLine = result.message ?? (result.queued ? "Saving your replay…" : "Replay saved")
        }
    }

    func applyPatch(_ patch: [String: Any]) {
        guard handle != nil else { return }
        applyOptimisticPatch(patch)
        pendingPatch.merge(patch) { _, new in new }
        patchGeneration &+= 1
        patchFlushTask?.cancel()

        let sinceLast = Date().timeIntervalSince(lastPatchFlush)
        let delayMs = sinceLast >= patchCoalesceWindow
            ? patchLeadingDelayMs
            : max(patchLeadingDelayMs, Int((patchCoalesceWindow - sinceLast) * 1000))
        patchFlushTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(delayMs))
            if Task.isCancelled { return }
            self?.flushPendingPatch()
        }
    }

    private func flushPendingPatch() {
        guard handle != nil, !pendingPatch.isEmpty,
              let data = try? JSONSerialization.data(withJSONObject: pendingPatch),
              let json = String(data: data, encoding: .utf8) else { return }
        lastPatchFlush = Date()
        let flushedGeneration = patchGeneration
        offload({ handle in json.withCString { rewinder_update_settings(handle, $0) } }) { [weak self] text in
            guard let self else { return }
            if let s: Settings = self.decodeEnvelopeText(text, label: "settings") {
                self.settings = s
            }
            if self.patchGeneration == flushedGeneration {
                self.pendingPatch = [:]
            }
            self.refreshStateAsync()
        }
    }

    private func applyOptimisticPatch(_ patch: [String: Any]) {
        guard let current = settings,
              let curData = try? encoder.encode(current),
              var dict = (try? JSONSerialization.jsonObject(with: curData)) as? [String: Any]
        else { return }
        dict.merge(patch) { _, new in new }
        guard let mergedData = try? JSONSerialization.data(withJSONObject: dict),
              let merged = try? decoder.decode(Settings.self, from: mergedData)
