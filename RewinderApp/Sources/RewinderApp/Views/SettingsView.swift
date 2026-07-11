import SwiftUI
import AppKit
import Carbon.HIToolbox

struct SettingsView: View {
    @Bindable var engine: RewinderEngine
    @State private var draft: Settings
    @State private var showAdvanced = false
    @State private var showTroubleshooting = false
    @State private var showResetConfirmation = false
    @State private var didJustSave = false
    @AppStorage("saveSoundEnabled") private var saveSoundEnabled = true
    @AppStorage("closeKeepsDockIcon") private var closeKeepsDockIcon = true
    @AppStorage("minimizeOnBufferStart") private var minimizeOnBufferStart = false
    @State private var draftSaveSound: Bool
    @State private var draftCloseKeepsDock: Bool
    @State private var draftMinimizeOnBufferStart: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let scrollTarget: String?

    init(engine: RewinderEngine, initial: Settings, scrollTarget: String? = nil) {
        self.engine = engine
        self.scrollTarget = scrollTarget
        _draft = State(initialValue: initial)
        _draftSaveSound = State(
            initialValue: UserDefaults.standard.object(forKey: "saveSoundEnabled") as? Bool ?? true
        )
        _draftCloseKeepsDock = State(
            initialValue: UserDefaults.standard.object(forKey: "closeKeepsDockIcon") as? Bool ?? true
        )
        _draftMinimizeOnBufferStart = State(
            initialValue: UserDefaults.standard.bool(forKey: "minimizeOnBufferStart")
        )
    }

    private static let audioModes = [
        ("system_only", "System audio"),
        ("system_plus_mic", "System + mic"),
        ("video_only", "Video only"),
    ]
    private static let micBackends = [
        ("auto", "Automatic (recommended)"),
        ("sck_native", "ScreenCaptureKit"),
        ("avcapture", "AVCapture"),
        ("voice_isolation", "Apple Voice Isolation (echo cancellation)"),
    ]
    private static let savePathModes = [
        ("instant_mp4", "Instant"),
        ("smooth", "Smooth"),
        ("adaptive", "Adaptive"),
        ("fast", "Fast"),
    ]
    private static let audioSaveModes = [
        ("smooth", "Smooth"),
        ("fast", "Fast"),
        ("adaptive", "Adaptive"),
    ]
    private static let audioFallbacks = [
        ("system_only_fallback", "Fall back to system audio"),
        ("allow_video_only", "Allow video only"),
    ]
    private static let resolutions = Presets.resolutions

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(spacing: 16) {
                    recordingSection
                    audioSection.id("audio")
                    savingSection
                    shortcutsSection.id("shortcuts")
                    advancedCard
                    troubleshootingSection
                    resetButton
                }
                .padding(28)
                .frame(maxWidth: 560)
                .frame(maxWidth: .infinity)
            }
            .safeAreaInset(edge: .bottom) { saveBar }
            .navigationTitle("Settings")
            .onChange(of: engine.settings) { _, new in
                if let new { draft = new }
            }
            .onAppear {
                engine.refreshMicrophones()
                guard let scrollTarget else { return }
                DispatchQueue.main.async {
                    withAnimation(.easeInOut(duration: 0.25)) {
                        proxy.scrollTo(scrollTarget, anchor: .top)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func labeledRow<Control: View>(_ label: String,
                                           @ViewBuilder control: () -> Control) -> some View {
        HStack(spacing: 12) {
            Text(label)
            Spacer(minLength: 12)
            control()
        }
    }

    @ViewBuilder
    private func stackedRow<Control: View>(_ label: String,
                                           @ViewBuilder control: () -> Control) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            control()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func subHeader(_ title: String, _ icon: String) -> some View {
        HStack(spacing: 6) {
            Image(systemName: icon)
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(.secondary)
            Text(title)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
        }
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func autoMicLabel() -> String {
        let name = detectedDefaultMicName()
        return name == "Automatic" ? "Automatic" : "\(name) (Auto)"
    }

    private func sliderRow(
        _ label: String,
        value: Binding<Double>,
        range: ClosedRange<Double>,
        format: @escaping (Double) -> String
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(label).font(.subheadline).foregroundStyle(.secondary)
                Spacer(minLength: 8)
                Text(format(value.wrappedValue))
                    .font(.subheadline.monospacedDigit().weight(.medium))
                    .foregroundStyle(.primary)
            }
            Slider(value: value, in: range)
                .tint(Theme.accent)
                .labelsHidden()
        }
    }

    private func disclosureCard<Content: View>(
        _ title: String,
        icon: String,
        isExpanded: Binding<Bool>,
        @ViewBuilder content: () -> Content
    ) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) { isExpanded.wrappedValue.toggle() }
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: icon)
                        .font(.headline)
                        .symbolRenderingMode(.hierarchical)
                        .foregroundStyle(Theme.accent)
                    Text(title)
                        .font(.headline)
                    Spacer(minLength: 0)
                    Image(systemName: "chevron.right")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .rotationEffect(.degrees(isExpanded.wrappedValue ? 90 : 0))
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .pointerStyle(.link)

            if isExpanded.wrappedValue {
                content()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .glassChrome()
    }

    private var recordingSection: some View {
        SettingsCard(title: "Recording", icon: "video") {
            VStack(alignment: .leading, spacing: 6) {
                stackedRow("Replay duration") {
                    Picker("Replay duration", selection: $draft.replayDurationSecs) {
                        ForEach(Presets.durations, id: \.self) { Text("\($0)s").tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
                caption("How far back each saved clip reaches.")
            }

            VStack(alignment: .leading, spacing: 6) {
                stackedRow("Frame rate") {
                    Picker("Frame rate", selection: $draft.fps) {
                        ForEach(Presets.frameRates, id: \.self) { Text("\($0) fps").tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
                if draft.batteryGuardEnabled, draft.batteryMaxFps < draft.fps {
                    caption("Battery saver limits recording to \(draft.batteryMaxFps) fps on battery.")
                }
            }

            labeledRow("Video resolution") {
                Picker("Video resolution", selection: $draft.videoResolution) {
                    ForEach(Self.resolutions, id: \.self) { Text(verbatim: "\($0)p").tag($0) }
                }
                .labelsHidden()
            }

            let displays = DisplayDevice.connected()
            if displays.count > 1 {
                VStack(alignment: .leading, spacing: 6) {
                    labeledRow("Capture display") {
                        Picker("Capture display", selection: Binding(
                            get: {
                                let stored = draft.selectedDisplayId ?? ""
                                return displays.contains { $0.id == stored } ? stored : ""
                            },
                            set: { draft.selectedDisplayId = $0 }
                        )) {
                            Text(mainDisplayLabel(displays)).tag("")
                            ForEach(displays.filter { !$0.isMain }) { display in
                                Text(display.name).tag(display.id)
                            }
                        }
                        .labelsHidden()
                    }
                    caption("Which screen is recorded. Switching restarts capture briefly.")
                }
            }
        }
    }

    private func mainDisplayLabel(_ displays: [DisplayDevice]) -> String {
        guard let main = displays.first(where: { $0.isMain }) else { return "Main display" }
        return "\(main.name) (Main)"
    }

    private var audioSection: some View {
        SettingsCard(title: "Audio", icon: "speaker.wave.2") {
            VStack(alignment: .leading, spacing: 6) {
                Toggle("Capture microphone", isOn: Binding(
                    get: { draft.micEnabled },
                    set: { on in
                        draft.micEnabled = on
                        draft.audioMode = on ? "system_plus_mic" : "system_only"
                    }
                ))
                caption("Mix your voice into saved replays.")
            }

            sliderRow("System audio", value: Binding(
                get: { Double(draft.systemVolumePercent) },
                set: { draft.systemVolumePercent = Int($0.rounded()) }
            ), range: 0...100) { "\(Int($0.rounded()))%" }

            VStack(alignment: .leading, spacing: 6) {
                sliderRow("Microphone loudness", value: $draft.micMixGainDb, range: 0...18) {
                    "\(Int($0.rounded())) dB"
                }
                .disabled(!draft.micEnabled)
                caption("How loud your voice sits over system audio. Lower keeps it in front.")
            }

            VStack(alignment: .leading, spacing: 6) {
                labeledRow("Microphone") {
                    Picker("Microphone", selection: Binding(
                        get: { draft.selectedMicrophoneId ?? "" },
                        set: { draft.selectedMicrophoneId = $0 }
                    )) {
                        Text(autoMicLabel()).tag("")
                        ForEach(engine.microphones) { mic in
                            Text(mic.name).tag(mic.id)
                        }
                    }
                    .labelsHidden()
                    .disabled(!draft.micEnabled)
                }
                caption("Auto follows your Mac's current input device, like macOS does.")
            }

            VStack(alignment: .leading, spacing: 6) {
                Toggle("Reduce mic background noise", isOn: $draft.micNoiseSuppression)
                    .disabled(!draft.micEnabled)
                caption("AI noise removal for your mic: fans, keyboard, room noise.")
            }
        }
    }

    private var savingSection: some View {
        SettingsCard(title: "Saving", icon: "square.and.arrow.down") {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    Image(systemName: "folder.fill")
                        .font(.title3)
                        .foregroundStyle(Theme.accent)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(saveFolderName)
                            .font(.body.weight(.medium))
                            .lineLimit(1)
                            .truncationMode(.middle)
                        Text(saveFolderPathDisplay)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                    Spacer(minLength: 8)
                    Button("Choose…") { chooseSaveFolder() }
                        .controlSize(.small)
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(.white.opacity(0.04), in: .rect(cornerRadius: 8))
                caption("Clips save here automatically.")

                VStack(alignment: .leading, spacing: 6) {
                    Toggle("Play sound when a clip saves", isOn: $draftSaveSound)
                        .padding(.top, 6)
                    caption("The confirmation chime after a successful save.")
                }

                VStack(alignment: .leading, spacing: 6) {
                    Toggle("Keep Rewinder in the Dock when the window closes", isOn: $draftCloseKeepsDock)
                        .padding(.top, 6)
                    caption("The close button never quits. When off, closing hides Rewinder in the menu bar.")
                }

                VStack(alignment: .leading, spacing: 6) {
                    Toggle("Minimize when the replay buffer starts", isOn: $draftMinimizeOnBufferStart)
                        .padding(.top, 6)
                    caption("The buffer won't start on launch. Start it yourself and the window minimizes automatically.")
                }
            }
        }
    }

    private var saveFolderName: String {
        let name = URL(fileURLWithPath: draft.outputDir).lastPathComponent
        return name.isEmpty ? "Choose a folder" : name
    }

    private var saveFolderPathDisplay: String {
        draft.outputDir.isEmpty
            ? "No folder selected"
            : (draft.outputDir as NSString).abbreviatingWithTildeInPath
    }

    private func chooseSaveFolder() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose"
        panel.message = "Choose where Rewinder saves your clips"
        if !draft.outputDir.isEmpty {
            panel.directoryURL = URL(fileURLWithPath: draft.outputDir)
        }
        if panel.runModal() == .OK, let url = panel.url {
            draft.outputDir = url.path
        }
    }

    private var shortcutsSection: some View {
        SettingsCard(title: "Shortcuts", icon: "keyboard") {
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("Save replay hotkey")
                    Spacer()
                    HotkeyRecorder(hotkey: $draft.hotkey)
                }
                caption("Click, then press your keys. Works from any app, even in fullscreen apps.")
            }
        }
    }

    private var advancedCard: some View {
        disclosureCard("Advanced", icon: "slider.horizontal.3", isExpanded: $showAdvanced) {
            VStack(alignment: .leading, spacing: 18) {
                advancedRecording
                Divider().overlay(.white.opacity(0.08))
                advancedAudio
                Divider().overlay(.white.opacity(0.08))
                advancedSaving
                Divider().overlay(.white.opacity(0.08))
                qualityGroup
            }
        }
    }

    private var advancedRecording: some View {
        VStack(alignment: .leading, spacing: 12) {
            subHeader("Recording", "video")
            Stepper("Buffer duration: \(draft.bufferDurationSecs)s",
                    value: $draft.bufferDurationSecs, in: 1...600)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var advancedAudio: some View {
        VStack(alignment: .leading, spacing: 12) {
            subHeader("Audio", "speaker.wave.2")
            Toggle("Exclude app audio", isOn: $draft.excludeCurrentProcessAudio)
            labeledRow("Audio mode") {
                Picker("Audio mode", selection: $draft.audioMode) {
                    ForEach(Self.audioModes, id: \.0) { Text($0.1).tag($0.0) }
                }
                .labelsHidden()
            }
            labeledRow("Mic backend") {
                Picker("Mic backend", selection: $draft.micCaptureBackend) {
                    ForEach(Self.micBackends, id: \.0) { Text($0.1).tag($0.0) }
                }
                .labelsHidden()
                .disabled(!draft.micEnabled)
            }
            Stepper("Audio bitrate: \(draft.audioBitrateKbps) kbps",
                    value: $draft.audioBitrateKbps, in: 64...512, step: 16)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var advancedSaving: some View {
        VStack(alignment: .leading, spacing: 12) {
            subHeader("Saving", "square.and.arrow.down")
            labeledRow("Save path mode") {
                Picker("Save path mode", selection: $draft.savePathMode) {
                    ForEach(Self.savePathModes, id: \.0) { Text($0.1).tag($0.0) }
                }
                .labelsHidden()
            }
            labeledRow("Audio save mode") {
                Picker("Audio save mode", selection: $draft.audioSaveMode) {
                    ForEach(Self.audioSaveModes, id: \.0) { Text($0.1).tag($0.0) }
                }
                .labelsHidden()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var qualityGroup: some View {
        VStack(alignment: .leading, spacing: 12) {
            subHeader("Quality", "bolt")
            stackedRow("Quality policy") {
                Picker("Quality policy", selection: $draft.qualityPolicy) {
                    Text("Adaptive").tag("adaptive_recover")
                    Text("Strict").tag("strict")
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            stackedRow("Quality preference") {
                Picker("Quality preference", selection: $draft.qualityPreference) {
                    Text("Quality").tag("prefer_quality")
                    Text("Smoothness").tag("prefer_smoothness")
                }
                .pickerStyle(.segmented)
                .labelsHidden()
            }

            Toggle("Battery saver", isOn: $draft.batteryGuardEnabled)

            if draft.batteryGuardEnabled {
                stackedRow("Battery frame-rate cap") {
                    Picker("Battery frame-rate cap", selection: $draft.batteryMaxFps) {
                        ForEach([24, 30, 60], id: \.self) { Text("\($0) fps").tag($0) }
                    }
                    .pickerStyle(.segmented)
                    .labelsHidden()
                }
            }

            labeledRow("Audio fallback policy") {
                Picker("Audio fallback policy", selection: $draft.audioFallbackPolicy) {
                    ForEach(Self.audioFallbacks, id: \.0) { Text($0.1).tag($0.0) }
                }
                .labelsHidden()
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var troubleshootingSection: some View {
        disclosureCard("Troubleshooting", icon: "stethoscope", isExpanded: $showTroubleshooting) {
            VStack(alignment: .leading, spacing: 12) {
                if let state = engine.engineState {
                    ForEach(healthBadges(state), id: \.label) { badge in
                        HealthBadge(label: badge.label, value: badge.value, tone: badge.tone)
                    }
                }
                Button("Copy diagnostics") {
                    copyDiagnostics()
                }
                .buttonStyle(.glass)
                .pointerStyle(.link)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var saveBar: some View {
        saveButton
            .frame(maxWidth: 300)
            .opacity(savePhase == .clean ? 0 : 1)
            .offset(y: reduceMotion ? 0 : (savePhase == .clean ? 14 : 0))
            .allowsHitTesting(savePhase != .clean)
            .animation(reduceMotion ? nil : .spring(duration: 0.35, bounce: 0.2), value: savePhase)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            .onChange(of: engine.isSubmittingSettings) { wasSubmitting, nowSubmitting in
                guard wasSubmitting, !nowSubmitting, draft == engine.settings else { return }
                didJustSave = true
                Task {
                    try? await Task.sleep(for: .seconds(1.2))
                    didJustSave = false
                }
            }
    }

    private var resetButton: some View {
        Button("Reset to default settings") {
            showResetConfirmation = true
        }
        .buttonStyle(.plain)
        .font(.subheadline)
        .foregroundStyle(.red)
        .pointerStyle(.link)
        .disabled(engine.isSubmittingSettings)
        .confirmationDialog(
            "Reset all settings to their defaults? This also resets the save folder and hotkey.",
            isPresented: $showResetConfirmation,
            titleVisibility: .visible
        ) {
            Button("Reset to default settings", role: .destructive) { resetToDefaults() }
            Button("Cancel", role: .cancel) {}
        }
    }

    private func resetToDefaults() {
        guard let defaults = engine.defaultSettings() else { return }
        draft = defaults
        draftSaveSound = true
        saveSoundEnabled = true
        draftCloseKeepsDock = true
        closeKeepsDockIcon = true
        draftMinimizeOnBufferStart = false
        minimizeOnBufferStart = false
        engine.submitSettings(defaults)
    }

    private enum SavePhase: Equatable { case clean, dirty, saving, saved }

    private var savePhase: SavePhase {
        if engine.isSubmittingSettings { return .saving }
        if didJustSave { return .saved }
        let engineClean = draft == engine.settings
        let appPrefsClean = draftSaveSound == saveSoundEnabled
            && draftCloseKeepsDock == closeKeepsDockIcon
            && draftMinimizeOnBufferStart == minimizeOnBufferStart
        return engineClean && appPrefsClean ? .clean : .dirty
    }

    private var saveTitle: String {
        switch savePhase {
        case .saving: return "Saving…"
        case .saved: return "Saved"
        case .clean, .dirty: return "Save settings"
        }
    }

    private var saveButtonLabel: some View {
        HStack(spacing: 7) {
            switch savePhase {
            case .saving:
                ProgressView().controlSize(.small)
            case .saved:
                Image(systemName: "checkmark")
                    .fontWeight(.semibold)
                    .transition(.scale.combined(with: .opacity))
                    .symbolEffect(.bounce, value: didJustSave)
            case .clean, .dirty:
                EmptyView()
            }
            Text(saveTitle)
                .contentTransition(.opacity)
        }
        .frame(maxWidth: .infinity)
    }

    private var saveButton: some View {
        Button {
            saveSoundEnabled = draftSaveSound
            closeKeepsDockIcon = draftCloseKeepsDock
            minimizeOnBufferStart = draftMinimizeOnBufferStart
            engine.submitSettings(draft)
        } label: {
            saveButtonLabel
        }
        .buttonStyle(.glassProminent)
        .controlSize(.large)
        .tint(savePhase == .saved ? .green : Theme.accent)
        .pointerStyle(.link)
        .disabled(savePhase != .dirty)
        .animation(reduceMotion ? nil : .spring(duration: 0.35, bounce: 0.2), value: savePhase)
    }

    private struct Badge { let label: String; let value: String; let tone: Tone }

    private func healthBadges(_ s: EngineState) -> [Badge] {
        var out: [Badge] = []
        out.append(Badge(
            label: "Screen recording",
            value: s.permission.screenRecordingGranted ? "Granted" : "Denied",
            tone: s.permission.screenRecordingGranted ? .success : .danger))

        let micValue: String
        let micTone: Tone
        if s.micPermissionStatus == "denied" { micValue = "Denied"; micTone = .danger }
        else if s.micAttachState == "live" { micValue = "Recording"; micTone = .success }
        else if s.micAttachState == "degraded" { micValue = "Disconnected"; micTone = .warning }
        else { micValue = "Off"; micTone = .neutral }
        out.append(Badge(label: "Microphone", value: micValue, tone: micTone))

        out.append(Badge(
            label: "System audio",
            value: s.systemAudioPathReady ? "Ready" : "Not ready",
            tone: s.systemAudioPathReady ? .success : .neutral))

        let (capValue, capTone): (String, Tone)
        switch s.captureHealth {
        case "running": (capValue, capTone) = ("Running", .success)
        case "degraded": (capValue, capTone) = ("Degraded", .warning)
        case "starting", "restarting": (capValue, capTone) = ("Starting", .accent)
        default: (capValue, capTone) = ("Stopped", .neutral)
        }
        out.append(Badge(label: "Capture", value: capValue, tone: capTone))

        if let err = s.lastError ?? s.armBlocker {
            out.append(Badge(label: "Last error", value: err, tone: .danger))
        }

        let (hkValue, hkTone): (String, Tone)
        switch s.hotkeyStatus {
        case "conflict": (hkValue, hkTone) = ("Conflict", .warning)
        case "invalid": (hkValue, hkTone) = ("Invalid", .danger)
        case "fallback": (hkValue, hkTone) = ("Fallback", .warning)
        default: (hkValue, hkTone) = (formatHotkey(s.settings.hotkey), .success)
        }
        out.append(Badge(label: "Shortcut", value: hkValue, tone: hkTone))
        return out
    }

    private func copyDiagnostics() {
        guard let s = engine.engineState else { return }
        var lines: [String] = []
        lines.append("Rewinder diagnostics")
        lines.append("status: \(engine.statusLine)")
        lines.append("lifecycle: \(s.lifecycleState)")
        lines.append("capture: \(s.captureHealth)  audio: \(s.audioHealth)")
        lines.append("armed: \(s.isArmed)  saving: \(s.isSaving)")
        lines.append("screenRecording: \(s.permission.screenRecordingGranted)")
        lines.append("outputDirWritable: \(s.permission.outputDirWritable)")
        lines.append("mic: \(s.micPermissionStatus) / \(s.micAttachState)")
        lines.append("hotkey: \(s.settings.hotkey) (\(s.hotkeyStatus))")
        lines.append("resolution: \(s.effectiveVideoResolution)p @ \(s.effectiveFps)fps")
        lines.append("buffer: \(Int(s.replayFillSecs))/\(Int(s.replayTargetSecs))s")
        if let err = s.lastError { lines.append("lastError: \(err)") }
        let text = lines.joined(separator: "\n")
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }
}
