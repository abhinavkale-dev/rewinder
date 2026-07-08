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

    private var leadingIcon: String {
        stage == .granted ? "checkmark.circle.fill" : icon
    }

    private var displayColor: Color {
        stage == .granted ? Theme.success : tone.color
    }

    var body: some View {
        Button { action() } label: {
            HStack(spacing: 9) {
                Image(systemName: leadingIcon)
                    .font(.callout)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(displayColor)
                    .contentTransition(.symbolEffect(.replace))
                    .symbolEffect(.bounce, value: stage == .granted)
                Text(title)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .contentTransition(.opacity)
                trailing
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .glassEffect(.regular, in: .capsule)
            .overlay(Capsule().strokeBorder(displayColor.opacity(0.28), lineWidth: 1))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .pointerStyle(stage == .granted ? .default : .link)
        .disabled(stage == .granted)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityTitle.isEmpty ? title : accessibilityTitle)
        .accessibilityHint(accessibilityHint)
        .accessibilityValue(stage == .checking ? "Checking" : (stage == .granted ? "Granted" : ""))
    }

    @ViewBuilder private var trailing: some View {
        switch stage {
        case .idle:
            Image(systemName: "arrow.forward")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
        case .checking:
            ProgressView().controlSize(.small)
        case .granted:
            EmptyView()
        }
    }
}

enum PermissionStatusIcon {
    enum Status { case granted, needed, off }
}

struct HealthBadge: View {
    let label: String
    let value: String
    let tone: Tone

    var body: some View {
        HStack {
            Text(label).foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.callout.weight(.medium))
                .foregroundStyle(tone.color)
        }
    }
}
