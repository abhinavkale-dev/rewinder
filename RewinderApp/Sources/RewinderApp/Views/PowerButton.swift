import SwiftUI

struct PowerButton: View {
    let phase: HomePhase
    let tone: Tone
    let progress: Double
    var nudge: Int = 0
    let action: @MainActor () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    @State private var nudgePulse = false

    private var ringActive: Bool {
        switch phase {
        case .building, .protected, .saving: return true
        default: return false
        }
    }

    private var morph: Animation? {
        reduceMotion ? nil : .spring(response: 0.42, dampingFraction: 0.82)
    }

    private var accessibility: String {
        switch phase {
        case .off: return "Turn replay on"
        case .permission: return "Screen Recording permission needed"
        case .saving: return "Saving your replay"
        case .protected: return "Protected. Turn replay off"
        case .building, .starting: return "Replay on. Turn off"
        }
    }

    var body: some View {
        Button { action() } label: {
            ZStack {
                Circle()
                    .stroke(Color.secondary.opacity(0.18), lineWidth: 7)
                Circle()
                    .trim(from: 0, to: ringActive ? progress : 0)
                    .stroke(tone.color, style: StrokeStyle(lineWidth: 7, lineCap: .round))
                    .rotationEffect(.degrees(-90))
                    .animation(reduceMotion ? nil : .linear(duration: 1), value: progress)
                RewinderRMark(color: phase == .off ? Color.secondary : tone.color, height: 66)
            }
            .frame(width: 156, height: 156)
            .padding(14)
            .glassEffect(.regular.interactive(), in: .circle)
            .animation(morph, value: phase)
            .scaleEffect(nudgePulse ? 1.03 : 1)
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .pointerStyle(.link)
        .accessibilityLabel(accessibility)
        .onChange(of: nudge) { _, _ in pulseOnce() }
    }

    private func pulseOnce() {
        guard !reduceMotion else { return }
        withAnimation(.spring(duration: 0.2, bounce: 0.35)) { nudgePulse = true }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(180))
            withAnimation(.spring(duration: 0.32, bounce: 0.2)) { nudgePulse = false }
        }
    }
}
