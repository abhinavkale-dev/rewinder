import SwiftUI

struct LoadingView: View {
    let errorText: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private enum Timing {
        static let mark = 450
        static let wordmark = 800
    }

    private enum Stage: Int, Comparable {
        case hidden = 0, track, mark, wordmark
        static func < (lhs: Stage, rhs: Stage) -> Bool { lhs.rawValue < rhs.rawValue }
    }

    @State private var stage: Stage = .hidden

    private var staticReveal: Bool { reduceMotion || errorText != nil }

    var body: some View {
        ZStack {
            Theme.appBackground.ignoresSafeArea()

            VStack(spacing: 18) {
                ring
                    .opacity(stage >= .track ? 1 : 0)
                    .scaleEffect(stage >= .track ? 1 : 0.95)
                    .animation(.easeOut(duration: 0.25), value: stage >= .track)

                Text("Rewinder")
                    .font(.system(size: 30, weight: .bold))
                    .foregroundStyle(.primary)
                    .opacity(stage >= .wordmark ? 1 : 0)
                    .animation(.easeOut(duration: 0.2), value: stage >= .wordmark)

                statusRow
                    .opacity(stage >= .wordmark ? 1 : 0)
                    .animation(.easeOut(duration: 0.2), value: stage >= .wordmark)
            }
        }
        .task { await runStoryboard() }
        .onChange(of: errorText) { _, error in
            if error != nil { stage = .wordmark }
        }
    }

    private var ring: some View {
        ZStack {
            BufferRingSweep(lineWidth: 7, completeImmediately: errorText != nil)
            RewinderRMark(color: Theme.accent, height: 66)
                .opacity(stage >= .mark ? 1 : 0)
                .scaleEffect(stage >= .mark ? 1 : 0.92)
                .animation(.easeOut(duration: 0.25), value: stage >= .mark)
        }
        .frame(width: 156, height: 156)
        .padding(14)
    }

    private func runStoryboard() async {
        if staticReveal {
            withAnimation(.easeOut(duration: 0.3)) { stage = .wordmark }
            return
        }

        stage = .track
        try? await Task.sleep(for: .milliseconds(Timing.mark))
        if Task.isCancelled { return }
        stage = .mark
        try? await Task.sleep(for: .milliseconds(Timing.wordmark - Timing.mark))
        if Task.isCancelled { return }
        stage = .wordmark
    }

    @ViewBuilder
    private var statusRow: some View {
        if let errorText {
            Text(errorText)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        } else {
            Text("Starting up…")
                .font(.callout)
                .foregroundStyle(.secondary)
        }
    }
}
