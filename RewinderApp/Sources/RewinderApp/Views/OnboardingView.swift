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
                subtitle: "Optional — mix your voice into saved replays.",
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
