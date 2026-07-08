import SwiftUI

enum Theme {
    static let accent = Color(.sRGB, red: 0.184, green: 0.361, blue: 0.941, opacity: 1)
    static let success = Color(.sRGB, red: 0.110, green: 0.616, blue: 0.373, opacity: 1)
    static let warning = Color(.sRGB, red: 0.851, green: 0.541, blue: 0.043, opacity: 1)
    static let danger = Color(.sRGB, red: 0.898, green: 0.282, blue: 0.302, opacity: 1)

    static let appBackgroundNS = NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(srgbRed: 0.110, green: 0.114, blue: 0.129, alpha: 1)
            : NSColor(srgbRed: 0.961, green: 0.965, blue: 0.973, alpha: 1)
    }
    static let appBackground = Color(nsColor: appBackgroundNS)
}

enum Tone {
    case neutral, accent, success, warning, danger

    var color: Color {
        switch self {
        case .neutral: return .secondary
        case .accent: return Theme.accent
        case .success: return Theme.success
        case .warning: return Theme.warning
        case .danger: return Theme.danger
        }
    }
}

func formatHotkey(_ raw: String) -> String {
    let parts = raw.split(separator: "+").map { $0.trimmingCharacters(in: .whitespaces) }
    return parts.map { token in
        HotkeyModifier.from(alias: token)?.glyph ?? token.uppercased()
    }.joined()
}
