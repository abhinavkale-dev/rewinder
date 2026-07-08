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
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private let scrollTarget: String?

    init(engine: RewinderEngine, initial: Settings, scrollTarget: String? = nil) {
        self.engine = engine
        self.scrollTarget = scrollTarget
        _draft = State(initialValue: initial)
    }

    private static let audioModes = [
        ("system_only", "System audio"),
        ("system_plus_mic", "System + mic"),
        ("video_only", "Video only"),
    ]
    private static let micBackends = [
        ("auto", "Automatic"),
        ("sck_native", "ScreenCaptureKit"),
        ("avcapture", "AVCapture"),
        ("voice_isolation", "Apple Voice Isolation (calls; lowers system volume)"),
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
                caption("AI noise removal for your mic — fans, keyboard, room noise.")
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
            }
        }
    }
