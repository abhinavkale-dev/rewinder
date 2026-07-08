import SwiftUI
import AppKit
import Carbon.HIToolbox

struct HotkeyRecorder: View {
    @Binding var hotkey: String
    @State private var isRecording = false
    @State private var monitor: Any?

    var body: some View {
        Button {
            isRecording ? stopRecording() : startRecording()
        } label: {
            Text(label)
                .font(.body.monospaced().weight(.semibold))
                .foregroundStyle(isRecording ? Theme.accent : .primary)
                .frame(minWidth: 120)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .contentShape(Rectangle())
        }
        .buttonStyle(.bordered)
        .help(isRecording
            ? "Press your key combination, or Esc to cancel"
            : "Click, then press your shortcut")
        .onDisappear(perform: stopRecording)
    }

    private var label: String {
        if isRecording { return "Press keys…" }
        let glyphs = formatHotkey(hotkey)
        return glyphs.isEmpty ? "Click to set" : glyphs
    }

    private func startRecording() {
        isRecording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { event in
            handle(event)
            return nil
        }
    }

    private func stopRecording() {
        isRecording = false
        if let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    private func handle(_ event: NSEvent) {
        if Int(event.keyCode) == kVK_Escape {
            stopRecording()
            return
        }
        guard let token = keyToken(for: event) else { return }

        let flags = event.modifierFlags
        var parts = HotkeyModifier.allCases
            .filter { flags.contains($0.eventFlag) }
            .map(\.token)
        guard !parts.isEmpty else { return }

        parts.append(token)
        hotkey = parts.joined(separator: "+")
        stopRecording()
    }

    private func keyToken(for event: NSEvent) -> String? {
        switch Int(event.keyCode) {
        case kVK_Space: return "Space"
        case kVK_Return, kVK_ANSI_KeypadEnter: return "Return"
        case kVK_Tab: return "Tab"
        default: break
        }
        guard let first = event.charactersIgnoringModifiers?.first,
              first.isLetter || first.isNumber else { return nil }
        return String(first).uppercased()
    }
}
