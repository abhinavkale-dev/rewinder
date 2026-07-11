import AppKit
import SwiftUI

struct HomeView: View {
    @Bindable var engine: RewinderEngine
    var onNavigate: (AppView, String?) -> Void = { _, _ in }

    private let clipLengthPresets = Presets.durations
    private let fpsPresets = Presets.frameRates
    private let resolutionPresets = Presets.resolutions

    @State private var activeStatusPopover: StatusPopover?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var fillAnchor: FillAnchor?

    @State private var lastRestartCount: Int?
    @State private var rebuildNote: String?

    @State private var savedConfirm = false
    @State private var savedResetTask: Task<Void, Never>?
    @GestureState private var savePressed = false

    private enum DeniedTiming {
        static let shake = 0.42
        static let ringNudgeDelayMs = 120
        static let hintIn = 0.15
        static let hintHold = 2.5
        static let hintOut = 0.3
    }
    @State private var deniedShakes = 0
    @State private var ringNudge = 0
    @State private var showReplayOffHint = false
    @State private var replayOffHintTask: Task<Void, Never>?

    @State private var showQualityHint = false
    @State private var qualityHintTask: Task<Void, Never>?

    @State private var guardFlash: String?
    @State private var guardFlashTask: Task<Void, Never>?

    private var state: EngineState? { engine.engineState }

    var body: some View {
        content
        .padding(28)
        .frame(maxWidth: 560)
        .frame(maxWidth: .infinity)
        .onAppear {
            seedAnchor()
            engine.refreshClips()
        }
        .onChange(of: captureLive) { _, live in
            fillAnchor = live ? FillAnchor(start: Date(), secs: backendFill) : nil
            lastRestartCount = state?.captureRestartCount
            rebuildNote = nil
        }
        .onChange(of: backendFill) { _, fill in advanceAnchor(to: fill) }
        .onChange(of: state?.captureRestartCount) { _, _ in advanceAnchor(to: backendFill) }
        .onChange(of: engine.lastGuardTransition?.sampledAtEpochMs) { _, _ in
            flashGuardTransition()
        }
        .task(id: pollActive) {
            guard pollActive else { return }
            while !Task.isCancelled {
                let interval: Duration = captureStartingUp ? .milliseconds(500) : .seconds(2)
                try? await Task.sleep(for: interval)
                if Task.isCancelled { return }
                engine.refreshStateAsync()
            }
        }
    }

    private var shouldPollFill: Bool {
        captureLive && currentFill(at: Date()) < replayTarget
    }

    private var captureStartingUp: Bool {
        let health = state?.captureHealth
        return health == "starting" || health == "restarting"
    }

    private var pollActive: Bool { shouldPollFill || captureStartingUp }

    private enum SaveButtonState: Equatable { case ready, saving, saved }

    private func saveButtonState(_ model: HomeModel) -> SaveButtonState {
        if savedConfirm { return .saved }
        if model.phase == .saving { return .saving }
        return .ready
    }

    private func saveButtonTitle(_ saveState: SaveButtonState, _ model: HomeModel) -> String {
        switch saveState {
        case .saved: return "Saved"
        case .saving: return "Saving…"
        case .ready: return "Save last \(Int(model.replayTarget))s"
        }
    }

    private func saveButtonIcon(_ saveState: SaveButtonState) -> String {
        saveState == .saved ? "checkmark.circle.fill" : "square.and.arrow.down"
    }

    private var clipsTodayCount: Int {
        let calendar = Calendar.current
        return engine.clips.filter {
            calendar.isDateInToday(Date(timeIntervalSince1970: $0.createdAtEpochMs / 1000))
        }.count
    }

    private func flashSaved() {
        savedResetTask?.cancel()
        savedConfirm = true
        savedResetTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(1.2))
            if Task.isCancelled { return }
            savedConfirm = false
        }
    }

    private func denySaveWhileOff() {
        Notifier.playCue(bundled: "save-denied", fallback: "Basso")
        if !reduceMotion {
            withAnimation(.easeInOut(duration: DeniedTiming.shake)) { deniedShakes += 1 }
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(DeniedTiming.ringNudgeDelayMs))
                ringNudge += 1
            }
        }
        flashReplayOffHint()
    }

    private func flashReplayOffHint() {
        replayOffHintTask?.cancel()
        withAnimation(reduceMotion ? nil : .easeOut(duration: DeniedTiming.hintIn)) {
            showReplayOffHint = true
        }
        replayOffHintTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(DeniedTiming.hintHold))
            if Task.isCancelled { return }
            withAnimation(reduceMotion ? nil : .easeOut(duration: DeniedTiming.hintOut)) {
                showReplayOffHint = false
            }
        }
    }

    private func flashGuardTransition() {
        guard let transition = engine.lastGuardTransition else { return }
        let message = transition.action == "step_up"
            ? "Full quality restored. Back to your chosen settings."
            : "Quality lowered to keep recording smooth."
        guardFlashTask?.cancel()
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.2)) { guardFlash = message }
        guardFlashTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(4))
            if Task.isCancelled { return }
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.3)) { guardFlash = nil }
        }
    }

    private func flashQualityHint() {
        qualityHintTask?.cancel()
        withAnimation(reduceMotion ? nil : .easeOut(duration: 0.2)) { showQualityHint = true }
        qualityHintTask = Task { @MainActor in
            try? await Task.sleep(for: .seconds(2.5))
            if Task.isCancelled { return }
            withAnimation(reduceMotion ? nil : .easeOut(duration: 0.3)) { showQualityHint = false }
        }
    }

    private var content: some View {
        VStack(spacing: 22) {
                if let state {
                    let model = HomeModel(
                        state: state,
                        displayFill: currentFill(at: Date()),
                        rebuildNote: rebuildNote
                    )

                    TimelineView(.periodic(from: .now, by: 1)) { context in
                        let displayFill = currentFill(at: context.date)
                        let live = HomeModel(
                            state: state,
                            displayFill: displayFill,
                            rebuildNote: rebuildNote
                        )

                        VStack(spacing: 22) {
                            VStack(spacing: 6) {
                                Text(live.headline)
                                    .font(.system(size: 24, weight: .semibold))
                                    .id(live.headline)
                                    .transition(.push(from: .bottom).combined(with: .opacity))
                                Text(live.subtext)
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                                    .multilineTextAlignment(.center)
                                    .contentTransition(.numericText())
                            }
                            .frame(maxWidth: .infinity)
                            .animation(
                                reduceMotion ? nil : .spring(response: 0.45, dampingFraction: 0.82),
                                value: live.headline
                            )
                            .animation(
                                reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.85),
                                value: live.subtext
                            )

                            PowerButton(
                                phase: live.phase,
                                tone: live.tone,
                                progress: live.bufferFull
                                    ? 1
                                    : (live.replayTarget > 0 ? min(displayFill / live.replayTarget, 1) : 0),
                                nudge: ringNudge
                            ) {
                                engine.setReplayEnabled(!live.replayEnabled)
                            }
                        }
                    }

                    let saveState = saveButtonState(model)
                    VStack(spacing: 10) {
                        Button {
                            if model.phase == .off {
                                denySaveWhileOff()
                            } else {
                                engine.saveReplay()
                            }
                        } label: {
                            HStack(spacing: 6) {
                                Image(systemName: saveButtonIcon(saveState))
                                    .contentTransition(.symbolEffect(.replace))
                                Text(saveButtonTitle(saveState, model))
                                    .contentTransition(.opacity)
                            }
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 4)
                        }
                        .buttonStyle(.glassProminent)
                        .controlSize(.large)
                        .pointerStyle(.link)
                        .tint(saveState == .saved ? Theme.success : Theme.accent)
                        .animation(reduceMotion ? nil : .smooth(duration: 0.3), value: saveState)
                        .scaleEffect(savePressed && !reduceMotion ? 0.97 : 1)
                        .animation(reduceMotion ? nil : .spring(duration: 0.18, bounce: 0.2), value: savePressed)
                        .modifier(ShakeEffect(animatableData: CGFloat(deniedShakes)))
                        .simultaneousGesture(
                            DragGesture(minimumDistance: 0)
                                .updating($savePressed) { _, pressed, _ in pressed = true }
                        )
                        .disabled(model.saveDisabled && saveState != .saved)
                        .onChange(of: engine.saveConfirmations) { _, _ in flashSaved() }

                        HStack(spacing: 6) {
                            Text("or press")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Button {
                                onNavigate(.settings, "shortcuts")
                            } label: {
                                Text(formatHotkey(state.settings.hotkey))
                                    .font(.caption.monospaced().weight(.semibold))
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 3)
                                    .glassEffect(.regular, in: .rect(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                            .pointerStyle(.link)
                            .help("Change this shortcut in Settings")
                        }

                        if showReplayOffHint {
                            HStack(alignment: .firstTextBaseline, spacing: 5) {
                                Image(systemName: "power")
                                Text("Replay is off. Tap the ring above to turn it on.")
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            .font(.caption)
                            .foregroundStyle(Theme.warning)
                            .multilineTextAlignment(.center)
                            .transition(.opacity)
                        }

                        if clipsTodayCount > 0 {
                            Label(
                                "^[\(clipsTodayCount) clip](inflect: true) saved today",
                                systemImage: "film.stack"
                            )
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .contentTransition(.numericText())
                            .transition(.opacity)
                        }
                    }
                    .frame(maxWidth: 320)
                    .animation(reduceMotion ? nil : .snappy, value: clipsTodayCount)

                    if !model.alerts.isEmpty {
                        PermissionChips(
                            alerts: model.alerts,
                            onGrant: { perform($0.kind) },
                            onRecheck: { engine.recheckPermissions() }
                        )
                    }

                    QualityPill(
                        clipLength: state.settings.replayDurationSecs,
                        fps: state.settings.fps,
                        resolution: state.settings.videoResolution,
                        displayedFps: displayedFps(state),
                        displayedResolution: displayedResolution(state),
                        fpsAutoLowered: isFpsAutoLowered(state),
                        resolutionAutoLowered: isResolutionAutoLowered(state),
                        clipLengthPresets: clipLengthPresets,
                        fpsPresets: fpsPresets,
                        resolutionPresets: resolutionPresets,
                        reduceMotion: reduceMotion,
                        engine: engine
                    )
                    .equatable()
                    .onChange(of: state.settings.fps) { _, _ in flashQualityHint() }
                    .onChange(of: state.settings.videoResolution) { _, _ in flashQualityHint() }

                    if let note = autoStepDownNote(state) {
                        HStack(alignment: .firstTextBaseline, spacing: 5) {
                            Image(systemName: "info.circle")
                            Text(note)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .font(.caption)
                        .foregroundStyle(Theme.warning)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 460)
                        .transition(.opacity)
                    } else if let guardFlash {
                        HStack(alignment: .firstTextBaseline, spacing: 5) {
                            Image(systemName: "checkmark.circle")
                            Text(guardFlash)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .font(.caption)
                        .foregroundStyle(Theme.success)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 460)
                        .transition(.opacity)
                    } else if showQualityHint {
                        HStack(alignment: .firstTextBaseline, spacing: 5) {
                            Image(systemName: "arrow.triangle.2.circlepath")
                            Text("Applying new quality. Your buffer refills for a moment.")
                                .fixedSize(horizontal: false, vertical: true)
                        }
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 460)
                        .transition(.opacity)
                    }

                    statusFooter(model)
                } else {
                    ProgressView()
                }
        }
    }

    private var captureLive: Bool {
        let health = state?.captureHealth
        return health == "running" || health == "degraded"
    }

    private var backendFill: Double { state?.replayFillSecs ?? 0 }

    private var replayTarget: Double {
        guard let settings = state?.settings else { return 30 }
        return settings.replayDurationSecs > 0
            ? Double(settings.replayDurationSecs)
            : (state?.replayTargetSecs ?? 30)
    }

    private func currentFill(at date: Date) -> Double {
        guard captureLive else { return 0 }
        let raw = fillAnchor?.fill(at: date) ?? backendFill
        return min(raw, replayTarget)
    }

    private func seedAnchor() {
        guard captureLive else {
            fillAnchor = nil
            return
        }
        if fillAnchor == nil {
            fillAnchor = FillAnchor(start: Date(), secs: backendFill)
            lastRestartCount = state?.captureRestartCount
        }
    }

    private func advanceAnchor(to backend: Double) {
        guard captureLive else { return }
        let now = Date()
        let projected = fillAnchor?.fill(at: now) ?? 0
        let restart = state?.captureRestartCount ?? 0
        let restarted = lastRestartCount.map { $0 != restart } ?? false
        lastRestartCount = restart

        if restarted {
            beginRebuild(to: backend, at: now, reason: restartReason())
            return
        }

        if backend > projected {
            fillAnchor = FillAnchor(start: now, secs: backend)
        }
    }

    private func beginRebuild(to backend: Double, at now: Date, reason: String) {
        rebuildNote = reason
        fillAnchor = FillAnchor(start: now, secs: backend)
    }

    private func restartReason() -> String {
        guard let state else { return "Capture restarted" }
        let lowered = (state.effectiveVideoResolution > 0 && state.effectiveVideoResolution < state.settings.videoResolution)
            || (state.effectiveFps > 0 && state.effectiveFps < state.settings.fps)
        return lowered ? "Quality adjusted to stay smooth" : "Capture restarted"
    }

    private func isFpsAutoLowered(_ state: EngineState) -> Bool {
        state.effectiveFps > 0 && state.effectiveFps < state.settings.fps
    }

    private func isResolutionAutoLowered(_ state: EngineState) -> Bool {
        state.effectiveVideoResolution > 0 && state.effectiveVideoResolution < state.settings.videoResolution
    }

    private func autoStepDownNote(_ state: EngineState) -> String? {
        let fpsLow = isFpsAutoLowered(state)
        let resLow = isResolutionAutoLowered(state)
        let bitrateLow = state.guardState == "protecting"
            && state.requestedVideoBitrateKbps > 0
            && state.effectiveVideoBitrateKbps < state.requestedVideoBitrateKbps
        guard fpsLow || resLow || bitrateLow else { return nil }

        if !fpsLow && !resLow {
            return "Bitrate auto-lowered to keep recording smooth. Restores when your Mac catches up."
        }

        var targets: [String] = []
        if fpsLow { targets.append("\(state.effectiveFps) fps") }
        if resLow { targets.append("\(state.effectiveVideoResolution)p") }
        let target = targets.joined(separator: " · ")

        if fpsLow, state.settings.batteryGuardEnabled,
           state.powerSource == "battery",
           state.effectiveFps <= state.settings.batteryMaxFps {
            return "Battery saver: \(target) to save power. Full quality when you plug in."
        }
        return "Auto-lowered to \(target) to avoid dropped frames. Restores when your Mac catches up."
    }

    private func displayedFps(_ state: EngineState) -> Int {
        state.effectiveFps > 0 ? state.effectiveFps : state.settings.fps
    }

    private func displayedResolution(_ state: EngineState) -> Int {
        state.effectiveVideoResolution > 0 ? state.effectiveVideoResolution : state.settings.videoResolution
    }

    private func perform(_ kind: AlertKind) {
        switch kind {
        case .resume: engine.resumeCapture()
        case .restart: engine.setReplayEnabled(true)
        case .screen: engine.grantScreenRecording()
        case .downloads: engine.grantOutputDir()
        case .mic: engine.grantMicrophone(openSettingsIfDenied: true)
        case .micDenied: engine.requestMicrophonePermission()
        }
    }

    private func statusFooter(_ model: HomeModel) -> some View {
        HStack(spacing: 14) {
            statusCheck(
                symbol: "display", label: "Screen",
                status: model.screenStatus, popover: .screen, model: model
            )
            Text("·").font(.subheadline.weight(.bold)).foregroundStyle(.secondary.opacity(0.5))
            statusCheck(
                symbol: model.micStatus == .off ? "mic.slash" : "mic.fill", label: "Mic",
                status: model.micStatus, popover: .mic, model: model
            )
        }
    }

    private func statusCheck(
        symbol: String,
        label: String,
        status: PermissionStatusIcon.Status,
        popover: StatusPopover,
        model: HomeModel
    ) -> some View {
        let tint: Color = {
            switch status {
            case .granted: return Theme.success
            case .needed:  return Theme.warning
            case .off:     return .secondary
            }
        }()
        let badge: String? = {
            switch status {
            case .granted: return "checkmark.circle.fill"
            case .needed:  return "exclamationmark.circle.fill"
            case .off:     return nil
            }
        }()
        return Button {
            if popover == .mic { engine.refreshMicrophones() }
            activeStatusPopover = popover
        } label: {
            HStack(spacing: 5) {
                Image(systemName: symbol)
                    .font(.subheadline)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(tint)
                    .contentTransition(.symbolEffect(.replace))
                Text(label)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                if let badge {
                    Image(systemName: badge)
                        .font(.caption)
                        .foregroundStyle(tint)
                        .symbolEffect(.bounce, value: status)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .pointerStyle(.link)
        .help(label)
        .popover(
            isPresented: Binding(
                get: { activeStatusPopover == popover },
                set: { if !$0 && activeStatusPopover == popover { activeStatusPopover = nil } }
            ),
            arrowEdge: .bottom
        ) {
            statusPopoverContent(popover, model: model)
                .frame(width: 300)
                .padding(16)
        }
    }

    @ViewBuilder
    private func statusPopoverContent(_ kind: StatusPopover, model: HomeModel) -> some View {
        switch kind {
        case .screen: screenControls(model)
        case .mic: micControls(model)
        }
    }

    private func cardHeader(_ title: String, _ icon: String) -> some View {
        CardHeader(title: title, icon: icon)
    }

    @ViewBuilder
    private func screenControls(_ model: HomeModel) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            cardHeader("Capture display", "display")
            if model.screenStatus == .granted {
                let displays = DisplayDevice.connected()
                let selected = state?.settings.selectedDisplayId
                ForEach(displays) { display in
                    let isOn = display.isEffectiveSelection(storedId: selected, in: displays)
                    Button {
                        engine.applyPatch(["selectedDisplayId": display.id])
                    } label: {
                        HStack(spacing: 10) {
                            Image(systemName: isOn ? "largecircle.fill.circle" : "circle")
                                .foregroundStyle(isOn ? Theme.accent : .secondary)
                            Text(display.name)
                                .foregroundStyle(.primary)
                                .lineLimit(1)
                            Spacer(minLength: 0)
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Text("Switching restarts capture briefly.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            } else {
                Text("Screen Recording is off, so capture can't run.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Button("Enable Screen Recording") { engine.grantScreenRecording() }
                    .buttonStyle(.glass)
                    .tint(Theme.warning)
            }
        }
    }

    @ViewBuilder
    private func micControls(_ model: HomeModel) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            cardHeader("Audio", "mic.fill")

            if let micEnabled = state?.settings.micEnabled, !micEnabled {
                Button("Turn on microphone") { engine.applyPatch(["micEnabled": true]) }
                    .buttonStyle(.glass)
                    .tint(Theme.accent)
            } else if model.micStatus == .needed {
                Button("Enable Microphone") { engine.grantMicrophone(openSettingsIfDenied: true) }
                    .buttonStyle(.glass)
                    .tint(Theme.warning)
            } else {
                HStack {
                    Text("Device").font(.subheadline).foregroundStyle(.secondary)
                    Spacer(minLength: 8)
                    Text(activeMicName())
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(.primary)
                        .lineLimit(1)
                }
                Button {
                    onNavigate(.settings, "audio")
                } label: {
                    Label("Audio settings", systemImage: "slider.horizontal.3")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.glass)
            }
        }
    }

    private func activeMicName() -> String {
        let id = state?.settings.selectedMicrophoneId ?? ""
        if !id.isEmpty, let name = engine.microphones.first(where: { $0.id == id })?.name {
            return name
        }
        return detectedDefaultMicName()
    }

}

private struct ShakeEffect: GeometryEffect {
    var travel: CGFloat = 8
    var cycles: CGFloat = 3
    var animatableData: CGFloat

    func effectValue(size: CGSize) -> ProjectionTransform {
        let progress = animatableData.truncatingRemainder(dividingBy: 1)
        guard progress > 0 else { return ProjectionTransform(.identity) }
        let x = travel * (1 - progress) * sin(progress * .pi * 2 * cycles)
        return ProjectionTransform(CGAffineTransform(translationX: x, y: 0))
    }
}
