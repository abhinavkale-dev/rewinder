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
