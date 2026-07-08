import SwiftUI
import AVFoundation

func detectedDefaultMicName() -> String {
    AVCaptureDevice.default(for: .audio)?.localizedName ?? "Automatic"
}

extension View {
    func glassChrome(cornerRadius: CGFloat = 16) -> some View {
        glassEffect(.regular, in: .rect(cornerRadius: cornerRadius))
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius)
                    .strokeBorder(.white.opacity(0.08), lineWidth: 1)
            )
    }
}

struct GlassCard<Content: View>: View {
    var cornerRadius: CGFloat = 16
    @ViewBuilder var content: Content

    var body: some View {
        content
            .padding(16)
            .glassChrome(cornerRadius: cornerRadius)
    }
}

struct CardHeader: View {
    let title: String
    let icon: String

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: icon)
                .font(.headline)
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(Theme.accent)
            Text(title)
                .font(.headline)
            Spacer(minLength: 0)
        }
    }
}

struct SettingsCard<Content: View>: View {
    let title: String
    let icon: String
    var cornerRadius: CGFloat = 16
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            CardHeader(title: title, icon: icon)
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .glassChrome(cornerRadius: cornerRadius)
    }
}

struct PermissionChip: View {
    enum Stage { case idle, checking, granted }

    let tone: Tone
    let icon: String
    let title: String
    let stage: Stage
    var accessibilityTitle: String = ""
    var accessibilityHint: String = ""
    let action: @MainActor () -> Void
