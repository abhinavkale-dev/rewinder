import AppKit
import Carbon.HIToolbox

enum HotkeyModifier: CaseIterable {
    case control, option, shift, command

    var token: String {
        switch self {
        case .control: return "Ctrl"
        case .option:  return "Option"
        case .shift:   return "Shift"
        case .command: return "Cmd"
        }
    }

    var glyph: String {
        switch self {
        case .control: return "⌃"
        case .option:  return "⌥"
        case .shift:   return "⇧"
        case .command: return "⌘"
        }
    }

    var carbonFlag: UInt32 {
        switch self {
        case .control: return UInt32(controlKey)
        case .option:  return UInt32(optionKey)
        case .shift:   return UInt32(shiftKey)
        case .command: return UInt32(cmdKey)
        }
    }

    var eventFlag: NSEvent.ModifierFlags {
        switch self {
        case .control: return .control
        case .option:  return .option
        case .shift:   return .shift
        case .command: return .command
        }
    }

    private var aliases: [String] {
        switch self {
        case .control: return ["ctrl", "control"]
        case .option:  return ["alt", "option", "opt"]
        case .shift:   return ["shift"]
        case .command: return ["cmd", "command", "cmdorctrl", "commandorcontrol", "super", "meta"]
        }
    }

    static func from(alias raw: String) -> HotkeyModifier? {
        let lower = raw.lowercased()
        return allCases.first { $0.aliases.contains(lower) }
    }
}

final class HotkeyManager {
    enum Mode { case primary, fallback, none }

    struct Registration {
        let hotkey: String
        let mode: Mode
    }

    private var hotKeyRef: EventHotKeyRef?
    private var eventHandler: EventHandlerRef?
    private var lastFired = Date.distantPast
    private let onFire: @MainActor () -> Void

    init(onFire: @escaping @MainActor () -> Void) {
        self.onFire = onFire
    }

    deinit {
        if let hotKeyRef { UnregisterEventHotKey(hotKeyRef) }
        if let eventHandler { RemoveEventHandler(eventHandler) }
    }

    @discardableResult
    func register(primary: String, fallbacks: [String]) -> Registration {
        installHandlerIfNeeded()
        unregister()

        var candidates: [String] = [primary]
        for fb in fallbacks where !candidates.contains(fb) {
            candidates.append(fb)
        }

        for (index, candidate) in candidates.enumerated() {
            guard let combo = HotkeyManager.parse(candidate) else { continue }
            var ref: EventHotKeyRef?
            let id = EventHotKeyID(signature: HotkeyManager.signature, id: 1)
            let status = RegisterEventHotKey(
                combo.keyCode, combo.modifiers, id,
                GetApplicationEventTarget(), 0, &ref
            )
            if status == noErr, let ref {
                hotKeyRef = ref
