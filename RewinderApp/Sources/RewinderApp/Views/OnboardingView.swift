import SwiftUI

struct OnboardingView: View {
    @Bindable var engine: RewinderEngine
    let onComplete: () -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private enum Step { case intro, getStarted, welcome }
    @State private var step: Step = .intro

    private enum IntroStage {
        static let hidden = 0
        static let asleep = 1
        static let halfBlink = 2
        static let shut = 3
        static let awake = 4
        static let glanceLeft = 5
        static let glanceRight = 6
        static let settled = 7
    }

    private enum IntroTiming {
        static let halfBlink = 600
        static let shut = 830
        static let wake = 1050
        static let glanceLeft = 1450
        static let glanceRight = 1900
        static let settle = 2350
        static let advance = 2900
        static let reducedAdvance = 1500
    }

    @State private var introStage = IntroStage.hidden
    @State private var advanceTask: Task<Void, Never>?

    @State private var continueShake: CGFloat = 0

    @State private var screenRequesting = false
    @State private var micRequesting = false
    @State private var screenTimeoutTask: Task<Void, Never>?
    @State private var micTimeoutTask: Task<Void, Never>?

    private let introHeight: CGFloat = 168
    private let welcomeHeight: CGFloat = 92
    private let welcomeMaxHeight: CGFloat = 580
    private let requestTimeout: Duration = .seconds(35)

    var body: some View {
        ZStack {
            Theme.appBackground.ignoresSafeArea()

            if step == .intro, introStage >= IntroStage.settled {
                introBackdrop
                    .transition(.opacity)
            }

            mainStack
                .padding(36)
        }
        .frame(minWidth: 460, minHeight: 560)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Rectangle())
        .onTapGesture { advance() }
        .onAppear { startIntro() }
        .onDisappear {
            advanceTask?.cancel()
            screenTimeoutTask?.cancel()
            micTimeoutTask?.cancel()
        }
        .onChange(of: screenGranted) { _, granted in
            if granted { resolveScreenRequest() }
        }
        .onChange(of: engine.engineState?.micPermissionStatus) { _, status in
            guard let status, status != "not_determined" else { return }
            resolveMicRequest()
        }
    }

    private var mainStack: some View {
        VStack(spacing: step == .intro ? 0 : 22) {
            RewinderOwlLogo(
                height: step == .welcome ? welcomeHeight : introHeight,
                eyeOpenness: owlEyeOpenness,
                pupilShift: owlPupilShift
            )
            .scaleEffect(owlEntranceScale, anchor: .center)
            .opacity(owlOpacity)
            .offset(y: owlLift)

            if step == .getStarted {
                getStartedPanel
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
            }

            if step == .welcome {
                brandLockup
                    .transition(.opacity)
                permissionRows
                    .transition(.opacity.combined(with: .move(edge: .bottom)))
                privacyNote
                    .transition(.opacity)
                Spacer(minLength: 12)
                continueButton
                    .transition(.opacity)
            }
        }
        .frame(
            maxWidth: 400,
            maxHeight: step == .welcome ? welcomeMaxHeight : .infinity,
            alignment: step == .welcome ? .top : .center
        )
        .frame(maxHeight: .infinity)
    }

    private var getStartedPanel: some View {
        VStack(spacing: 8) {
            Text("Welcome to Rewinder")
                .font(.system(size: 26, weight: .bold))
                .foregroundStyle(.primary)
            Text("Your screen's last moments, always ready to save.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)

            Button {
                advanceToWelcome()
            } label: {
                Text("Get Started")
                    .padding(.horizontal, 24)
                    .padding(.vertical, 4)
            }
            .buttonStyle(.glassProminent)
            .controlSize(.large)
            .tint(Theme.accent)
            .pointerStyle(.link)
            .padding(.top, 12)
        }
    }

    private var introBackdrop: some View {
        ZStack {
            RewindRipple(delay: 0)
            RewindRipple(delay: 0.18)
        }
        .frame(width: 260, height: 260)
        .allowsHitTesting(false)
    }

    private var brandLockup: some View {
        VStack(spacing: 6) {
            Text("Welcome to")
                .font(.title3)
                .foregroundStyle(.secondary)
            HStack(spacing: 8) {
                RewinderRMark(color: .primary, height: 30)
                Text("Rewinder")
                    .font(.system(size: 30, weight: .bold))
                    .foregroundStyle(.primary)
            }
        }
    }

    private var permissionRows: some View {
        VStack(spacing: 12) {
            permissionRow(
                icon: "display",
                title: "Screen Recording",
                subtitle: "Required, so Rewinder can capture your screen.",
                requestingSubtitle: "Waiting for you to enable it in System Settings…",
                granted: screenGranted,
                requesting: screenRequesting
            ) { startScreenRequest() }

            permissionRow(
                icon: "mic.fill",
                title: "Microphone",
                subtitle: "Optional. Mix your voice into saved replays.",
                requestingSubtitle: "Approve the system prompt…",
                granted: micGranted,
                requesting: micRequesting
            ) { startMicRequest() }
        }
        .animation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.85), value: screenGranted)
        .animation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.85), value: micGranted)
        .animation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.85), value: screenRequesting)
        .animation(reduceMotion ? nil : .spring(response: 0.4, dampingFraction: 0.85), value: micRequesting)
    }

    private func permissionRow(
        icon: String,
        title: String,
        subtitle: String,
        requestingSubtitle: String,
        granted: Bool,
        requesting: Bool,
        action: @escaping () -> Void
    ) -> some View {
        let isRequesting = requesting && !granted
        return HStack(spacing: 14) {
            Image(systemName: icon)
                .font(.system(size: 20, weight: .medium))
                .symbolRenderingMode(.hierarchical)
                .symbolEffect(.pulse, isActive: isRequesting && !reduceMotion)
                .foregroundStyle(granted ? Theme.success : Theme.accent)
                .frame(width: 44, height: 44)
                .glassEffect(.regular, in: .rect(cornerRadius: 12))

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(.primary)
                Text(isRequesting ? requestingSubtitle : subtitle)
                    .font(.caption)
                    .foregroundStyle(isRequesting ? Theme.accent : Color.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer(minLength: 8)

            if granted {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title2)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(Theme.success)
                    .transition(.scale.combined(with: .opacity))
            } else if isRequesting {
                ProgressView()
                    .controlSize(.small)
                    .transition(.scale.combined(with: .opacity))
            } else {
                Button("Allow", action: action)
                    .buttonStyle(.glass)
                    .tint(Theme.accent)
                    .pointerStyle(.link)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .glassChrome()
    }

    private func startScreenRequest() {
        engine.grantScreenRecording()
        screenRequesting = true
        screenTimeoutTask?.cancel()
        screenTimeoutTask = Task { @MainActor in
            try? await Task.sleep(for: requestTimeout)
            if Task.isCancelled { return }
            screenRequesting = false
        }
    }

    private func startMicRequest() {
        if engine.engineState?.micPermissionStatus == "denied" {
            engine.grantMicrophone(openSettingsIfDenied: true)
        } else {
            engine.requestMicrophonePermission()
        }
        micRequesting = true
        micTimeoutTask?.cancel()
        micTimeoutTask = Task { @MainActor in
            try? await Task.sleep(for: requestTimeout)
            if Task.isCancelled { return }
            micRequesting = false
        }
    }

    private func resolveScreenRequest() {
        screenTimeoutTask?.cancel()
        screenTimeoutTask = nil
        screenRequesting = false
    }

    private func resolveMicRequest() {
        micTimeoutTask?.cancel()
        micTimeoutTask = nil
        micRequesting = false
    }

    private var privacyNote: some View {
        Text("\(Image(systemName: "lock.shield"))  Your clips stay on your Mac. Rewinder runs fully offline and never collects or shares your data.")
            .font(.caption)
            .foregroundStyle(.secondary)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: 320)
            .padding(.top, 2)
    }

    private var continueButton: some View {
        Button {
            attemptContinue()
        } label: {
            Text("Continue")
                .frame(maxWidth: .infinity)
                .padding(.vertical, 4)
        }
        .buttonStyle(.glassProminent)
        .controlSize(.large)
        .tint(screenGranted ? Theme.accent : Theme.danger)
        .animation(reduceMotion ? nil : .easeInOut(duration: 0.25), value: screenGranted)
        .pointerStyle(.link)
        .modifier(ShakeEffect(animatableData: continueShake))
        .sensoryFeedback(.error, trigger: continueShake)
    }

    private func attemptContinue() {
        guard screenGranted else {
            if reduceMotion {
                continueShake += 1
            } else {
                withAnimation(.linear(duration: 0.45)) { continueShake += 1 }
            }
            return
        }
        if !micGranted {
            engine.applyPatch(["audioMode": "system_only", "micEnabled": false])
        }
        onComplete()
    }

    private var screenGranted: Bool {
        engine.engineState?.permission.screenRecordingGranted ?? false
    }

    private var micGranted: Bool {
        engine.engineState?.micPermissionStatus == "granted"
    }

    private var owlEyeOpenness: CGFloat {
        guard step == .intro else { return 1 }
        switch introStage {
        case IntroStage.hidden, IntroStage.asleep, IntroStage.shut: return 0
        case IntroStage.halfBlink: return 0.35
        default: return 1
        }
    }

    private var owlPupilShift: CGFloat {
        guard step == .intro else { return 0 }
        switch introStage {
        case IntroStage.glanceLeft: return -14
        case IntroStage.glanceRight: return 14
        default: return 0
        }
    }

    private var owlLift: CGFloat {
        guard step == .intro else { return 0 }
        if introStage == IntroStage.hidden { return 10 }
        return introStage < IntroStage.awake ? 6 : 0
    }

    private var owlEntranceScale: CGFloat {
        (step == .intro && introStage == IntroStage.hidden) ? 0.92 : 1
    }

    private var owlOpacity: Double {
        (step == .intro && introStage == IntroStage.hidden) ? 0 : 1
    }

    private func startIntro() {
        advanceTask?.cancel()
        advanceTask = Task { @MainActor in
            if reduceMotion {
                withAnimation(.easeOut(duration: 0.3)) { introStage = IntroStage.awake }
                if await hold(IntroTiming.reducedAdvance) { advance() }
                return
            }

            withAnimation(.easeOut(duration: 0.35)) { introStage = IntroStage.asleep }

            guard await hold(IntroTiming.halfBlink) else { return }
            withAnimation(.easeInOut(duration: 0.16)) { introStage = IntroStage.halfBlink }

            guard await hold(IntroTiming.shut - IntroTiming.halfBlink) else { return }
            withAnimation(.easeInOut(duration: 0.14)) { introStage = IntroStage.shut }

            guard await hold(IntroTiming.wake - IntroTiming.shut) else { return }
            withAnimation(.spring(duration: 0.5, bounce: 0.25)) { introStage = IntroStage.awake }

            guard await hold(IntroTiming.glanceLeft - IntroTiming.wake) else { return }
            withAnimation(.spring(response: 0.28, dampingFraction: 0.8)) { introStage = IntroStage.glanceLeft }

            guard await hold(IntroTiming.glanceRight - IntroTiming.glanceLeft) else { return }
            withAnimation(.spring(response: 0.28, dampingFraction: 0.8)) { introStage = IntroStage.glanceRight }

            guard await hold(IntroTiming.settle - IntroTiming.glanceRight) else { return }
            withAnimation(.spring(response: 0.3, dampingFraction: 0.85)) { introStage = IntroStage.settled }

            guard await hold(IntroTiming.advance - IntroTiming.settle) else { return }
            advance()
        }
    }

    private func hold(_ ms: Int) async -> Bool {
        try? await Task.sleep(for: .milliseconds(ms))
        return !Task.isCancelled
    }

    private func advance() {
        guard step == .intro else { return }
        advanceTask?.cancel()
        withAnimation(stepAnimation) { step = .getStarted }
    }

    private func advanceToWelcome() {
        guard step == .getStarted else { return }
        withAnimation(stepAnimation) { step = .welcome }
    }

    private var stepAnimation: Animation {
        reduceMotion
            ? .easeInOut(duration: 0.3)
            : .spring(response: 0.55, dampingFraction: 0.84)
    }
}

private struct ShakeEffect: GeometryEffect {
    var amount: CGFloat = 9
    var shakes: CGFloat = 3
    var animatableData: CGFloat

    func effectValue(size: CGSize) -> ProjectionTransform {
        let dx = amount * sin(animatableData * .pi * 2 * shakes)
        return ProjectionTransform(CGAffineTransform(translationX: dx, y: 0))
    }
}

private struct RewindRipple: View {
    let delay: Double
    @State private var converged = false

    var body: some View {
        Circle()
            .stroke(Theme.accent, lineWidth: 2)
            .scaleEffect(converged ? 0.85 : 1.45)
            .opacity(converged ? 0 : 0.55)
            .animation(.easeOut(duration: 0.7).delay(delay), value: converged)
            .onAppear { converged = true }
    }
}
