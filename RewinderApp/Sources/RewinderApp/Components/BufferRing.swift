import SwiftUI

struct BufferRingSweep: View {
    var lineWidth: CGFloat = 7
    var completeImmediately = false

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var headTotal = 0

    private static let segmentCount = 16
    private static let historyWindow = 12

    private enum Timing {
        static let sweepStartMs = 150
        static let sweepStepMs = 35
        static let idleStepMs = 110
    }

    private var staticReveal: Bool { reduceMotion || completeImmediately }

    var body: some View {
        ZStack {
            ForEach(0..<Self.segmentCount, id: \.self) { index in
                segmentArc(index)
                    .stroke(
                        Theme.accent,
                        style: StrokeStyle(lineWidth: lineWidth, lineCap: .round)
                    )
                    .rotationEffect(.degrees(-90))
                    .opacity(segmentOpacity(index))
                    .animation(.easeOut(duration: 0.18), value: headTotal)
            }
        }
        .task { await runSweep() }
    }

    private func segmentArc(_ index: Int) -> some Shape {
        let span = 1.0 / CGFloat(Self.segmentCount)
        let gap = 0.012
        return Circle()
            .trim(
                from: CGFloat(index) * span + gap,
                to: CGFloat(index + 1) * span - gap
            )
    }

    private func segmentOpacity(_ index: Int) -> Double {
        if staticReveal { return 1 }
        guard headTotal > index else { return 0 }
        let age = (headTotal - 1 - index) % Self.segmentCount
        return age < Self.historyWindow ? 1 : 0.35
    }

    private func runSweep() async {
        guard !staticReveal else {
            headTotal = Self.segmentCount
            return
        }
        try? await Task.sleep(for: .milliseconds(Timing.sweepStartMs))
        while headTotal < Self.segmentCount, !Task.isCancelled {
            headTotal += 1
            try? await Task.sleep(for: .milliseconds(Timing.sweepStepMs))
        }
        while !Task.isCancelled {
            try? await Task.sleep(for: .milliseconds(Timing.idleStepMs))
            headTotal += 1
        }
    }
}
