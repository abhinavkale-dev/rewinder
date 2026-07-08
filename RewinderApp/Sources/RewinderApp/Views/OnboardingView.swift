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
