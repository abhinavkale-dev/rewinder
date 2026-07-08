import SwiftUI

struct QualityPill: View, Equatable {
    let clipLength: Int
    let fps: Int
    let resolution: Int
    let displayedFps: Int
    let displayedResolution: Int
    let fpsAutoLowered: Bool
    let resolutionAutoLowered: Bool
    let clipLengthPresets: [Int]
    let fpsPresets: [Int]
    let resolutionPresets: [Int]
    let reduceMotion: Bool
    let engine: RewinderEngine

    nonisolated static func == (lhs: QualityPill, rhs: QualityPill) -> Bool {
        lhs.clipLength == rhs.clipLength
            && lhs.fps == rhs.fps
            && lhs.resolution == rhs.resolution
            && lhs.displayedFps == rhs.displayedFps
            && lhs.displayedResolution == rhs.displayedResolution
            && lhs.fpsAutoLowered == rhs.fpsAutoLowered
            && lhs.resolutionAutoLowered == rhs.resolutionAutoLowered
            && lhs.reduceMotion == rhs.reduceMotion
    }

    var body: some View {
        HStack(spacing: 4) {
            menu(value: "\(clipLength)s", isAuto: false) {
                Picker("Clip length", selection: patchBinding("replayDurationSecs", clipLength)) {
                    ForEach(clipLengthPresets, id: \.self) { Text("\($0)s").tag($0) }
                }
                .pickerStyle(.inline)
            }
            divider
            menu(value: "\(displayedFps) fps", isAuto: fpsAutoLowered) {
                Picker("Frame rate", selection: patchBinding("fps", fps)) {
                    ForEach(fpsPresets, id: \.self) { preset in
                        Text(rowLabel("\(preset) fps", active: fpsAutoLowered && preset == displayedFps)).tag(preset)
                    }
                }
                .pickerStyle(.inline)
                if fpsAutoLowered {
                    Text("Auto-lowered to protect capture — returns to \(fps) fps automatically.")
                }
            }
            divider
            menu(value: "\(displayedResolution)p", isAuto: resolutionAutoLowered) {
                Picker("Resolution", selection: patchBinding("videoResolution", resolution)) {
                    ForEach(resolutionPresets, id: \.self) { preset in
                        Text(rowLabel("\(preset)p", active: resolutionAutoLowered && preset == displayedResolution)).tag(preset)
                    }
                }
                .pickerStyle(.inline)
                if resolutionAutoLowered {
                    Text("Auto-lowered to protect capture — returns to \(resolution)p automatically.")
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 3)
        .glassEffect(.regular, in: .capsule)
        .overlay(Capsule().strokeBorder(.white.opacity(0.08), lineWidth: 1))
    }

    private var divider: some View {
        Text("·").font(.subheadline.weight(.bold)).foregroundStyle(.secondary.opacity(0.5))
    }

    private func patchBinding(_ key: String, _ current: Int) -> Binding<Int> {
        Binding(get: { current }, set: { engine.applyPatch([key: $0]) })
    }

    private func rowLabel(_ base: String, active: Bool) -> String {
        active ? "\(base) · now" : base
    }

    private func menu<Content: View>(
        value: String,
        isAuto: Bool,
        @ViewBuilder content: () -> Content
    ) -> some View {
        Menu {
            content()
        } label: {
            HStack(spacing: 4) {
                Text(value)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(isAuto ? Theme.warning : .primary)
                    .contentTransition(.numericText())
                if isAuto {
                    Image(systemName: "arrow.down.circle.fill")
                        .font(.caption2)
                        .foregroundStyle(Theme.warning)
                }
                Image(systemName: "chevron.down")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
            }
            .animation(reduceMotion ? nil : .snappy(duration: 0.32), value: value)
            .padding(.vertical, 8)
            .padding(.horizontal, 8)
            .contentShape(Rectangle())
        }
        .menuStyle(.button)
        .menuIndicator(.hidden)
        .buttonStyle(.plain)
        .pointerStyle(.link)
        .fixedSize()
    }
}
