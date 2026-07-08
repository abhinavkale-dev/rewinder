import SwiftUI
import AppKit

enum AppView {
    case home, settings, clips
}

enum AppWindowMetrics {
    static let size = CGSize(width: 520, height: 612)
}

struct ContentView: View {
    @Bindable var engine: RewinderEngine
    @AppStorage("hasCompletedOnboarding") private var completed = false
    @State private var showOnboarding = false
    @State private var splashHoldDone = false
    @State private var homeRevealed = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var showSplash: Bool {
        if engine.engineState == nil { return true }
        return !(splashHoldDone || reduceMotion || engine.bootError != nil)
    }

    var body: some View {
        ZStack {
            if engine.engineState != nil {
                RootView(engine: engine)
                    .opacity(homeRevealed ? 1 : 0)
                    .scaleEffect(reduceMotion || homeRevealed ? 1 : 0.985)
                    .offset(y: reduceMotion || homeRevealed ? 0 : 6)
            }

            if showSplash {
                LoadingView(errorText: engine.bootError)
                    .transition(.opacity)
            }

            if showOnboarding {
                OnboardingView(engine: engine) {
                    completed = true
                    withAnimation(.easeOut(duration: reduceMotion ? 0.25 : 0.3)) {
                        showOnboarding = false
                        homeRevealed = true
                    }
                }
                .transition(.opacity)
            }
        }
        .animation(.easeOut(duration: 0.25), value: showSplash)
        .frame(width: AppWindowMetrics.size.width, height: AppWindowMetrics.size.height)
        .onAppear {
            showOnboarding = !completed
            homeRevealed = completed
        }
        .task {
            try? await Task.sleep(for: .milliseconds(1_150))
            splashHoldDone = true
        }
    }
}

struct RootView: View {
    @Bindable var engine: RewinderEngine
    @State private var activeView: AppView = .home
    @State private var settingsScrollTarget: String? = nil

    var body: some View {
        NavigationStack {
            content
                .frame(minWidth: 460, minHeight: 560, alignment: .top)
                .background(backdrop)
                .background(WindowAccessor { window in
                    window.titlebarAppearsTransparent = true
                    window.styleMask.insert(.fullSizeContentView)
                })
                .overlay(alignment: .top) {
                    if activeView != .home { topEdgeFade }
                }
                .navigationTitle(title)
                .toolbar {
                    if activeView != .home {
                        ToolbarItem(placement: .navigation) {
                            Button {
                                show(.home)
                            } label: {
                                Image(systemName: "chevron.backward")
                            }
                            .pointerStyle(.link)
                            .help("Back to Home")
                        }
                    }
                    ToolbarItemGroup(placement: .primaryAction) {
                        navToggle(.settings, symbol: "gearshape", help: "Settings")
                        navToggle(.clips, symbol: "film", help: "Clips")
                    }
                }
                .onChange(of: engine.pendingNavigation) { _, target in
                    applyPendingNavigation(target)
                }
                .onAppear { applyPendingNavigation(engine.pendingNavigation) }
        }
