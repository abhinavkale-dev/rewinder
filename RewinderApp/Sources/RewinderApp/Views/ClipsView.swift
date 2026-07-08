import AppKit
import SwiftUI

struct ClipsView: View {
    @Bindable var engine: RewinderEngine
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private static let columns = [GridItem(.adaptive(minimum: 220), spacing: 16)]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                Text("Recent clips")
                    .font(.system(size: 26, weight: .semibold))

                if engine.clips.isEmpty {
                    emptyState
                } else {
                    LazyVGrid(columns: Self.columns, alignment: .leading, spacing: 16) {
                        ForEach(Array(engine.clips.enumerated()), id: \.element.id) { index, clip in
                            ClipCard(clip: clip, index: index, reduceMotion: reduceMotion)
                        }
                    }
                }
            }
            .padding(28)
            .frame(maxWidth: 980)
            .frame(maxWidth: .infinity)
        }
        .onAppear { engine.refreshClips() }
    }

    private var emptyState: some View {
        GlassCard {
            VStack(spacing: 10) {
                Image(systemName: "film")
                    .font(.system(size: 34))
                    .foregroundStyle(.secondary)
                Text("No clips yet")
                    .font(.headline)
                Text("Save a replay from Home (or your hotkey) and it will show up here.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
        }
    }
}

private struct ClipCard: View {
    let clip: ClipMetadata
    var index: Int = 0
    var reduceMotion: Bool = false
    @State private var thumbnail: NSImage?
    @State private var loadedDurationSecs: Double?
    @State private var shown = false

    var body: some View {
        Button(action: open) {
            VStack(alignment: .leading, spacing: 0) {
                thumbnailArea
                footer
            }
        }
        .buttonStyle(.plain)
        .pointerStyle(.link)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .glassEffect(.regular, in: .rect(cornerRadius: 14))
        .help(clip.path)
        .opacity(shown ? 1 : 0)
        .scaleEffect(shown ? 1 : 0.96)
        .offset(y: shown ? 0 : 8)
        .onAppear(perform: reveal)
        .task(id: clip.path) {
            let preview = await ClipThumbnailCache.preview(forPath: clip.path)
            thumbnail = preview.image
            loadedDurationSecs = preview.durationSecs
        }
    }

    private func reveal() {
        guard !shown else { return }
        if reduceMotion { shown = true; return }
        let delay = Double(min(index, 10)) * 0.045
        withAnimation(.easeOut(duration: 0.35).delay(delay)) { shown = true }
    }

    private func open() {
        NSWorkspace.shared.open(URL(fileURLWithPath: clip.path))
    }

    private var thumbnailArea: some View {
        Color.clear
            .aspectRatio(16.0 / 9.0, contentMode: .fit)
            .overlay {
                if let thumbnail {
                    Image(nsImage: thumbnail)
                        .resizable()
                        .scaledToFill()
                        .transition(.opacity)
                } else {
                    placeholder
                        .transition(.opacity)
                }
            }
            .animation(reduceMotion ? nil : .easeOut(duration: 0.3), value: thumbnail != nil)
            .clipped()
            .overlay(alignment: .center) {
                if thumbnail != nil {
                    playBadge
                }
            }
            .overlay(alignment: .bottomTrailing) {
                if let secs = durationSecs {
                    durationBadge(secs)
                }
            }
    }

    private var durationSecs: Double? {
        if clip.durationSecs > 0 { return clip.durationSecs }
        if let loadedDurationSecs, loadedDurationSecs > 0 { return loadedDurationSecs }
        return nil
    }

    private var placeholder: some View {
        ZStack {
            Theme.accent.opacity(0.15)
            Image(systemName: "play.fill")
                .font(.system(size: 26))
                .foregroundStyle(Theme.accent)
        }
    }

    private var playBadge: some View {
        Image(systemName: "play.fill")
            .font(.system(size: 15, weight: .semibold))
            .foregroundStyle(.white)
            .padding(11)
            .background(.black.opacity(0.45), in: .circle)
    }

    private func durationBadge(_ secs: Double) -> some View {
        Text(durationText(secs))
            .font(.caption2.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(.black.opacity(0.55), in: .capsule)
            .padding(8)
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(fileName)
                .font(.subheadline.weight(.semibold))
                .lineLimit(1)
                .truncationMode(.middle)
            HStack {
                Text(relativeTime)
                Spacer(minLength: 8)
                Text(sizeText)
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var fileName: String {
        let base = (clip.path as NSString).lastPathComponent
        return base.isEmpty ? clip.id : base
    }

    private var relativeTime: String {
        let now = Date().timeIntervalSince1970 * 1000
        let diff = now - clip.createdAtEpochMs
        if diff < 60_000 { return "Just now" }
        if diff < 3_600_000 { return "\(Int(diff / 60_000))m ago" }
        if diff < 86_400_000 { return "\(Int(diff / 3_600_000))h ago" }
        let date = Date(timeIntervalSince1970: clip.createdAtEpochMs / 1000)
        return date.formatted(date: .abbreviated, time: .shortened)
    }

    private func durationText(_ durationSecs: Double) -> String {
        let secs = Int(durationSecs.rounded())
        if secs >= 60 { return String(format: "%d:%02d", secs / 60, secs % 60) }
        return "\(secs)s"
    }

    private var sizeText: String {
        let bytes = Double(clip.sizeBytes)
        let units = ["B", "KB", "MB", "GB"]
        var value = bytes
        var unit = 0
        while value >= 1024, unit < units.count - 1 {
            value /= 1024
            unit += 1
        }
        return unit == 0 ? "\(Int(value)) \(units[unit])" : String(format: "%.1f %@", value, units[unit])
    }
}
