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
                return Registration(hotkey: candidate, mode: index == 0 ? .primary : .fallback)
            }
        }
        return Registration(hotkey: primary, mode: .none)
    }

    func unregister() {
        if let hotKeyRef {
            UnregisterEventHotKey(hotKeyRef)
            self.hotKeyRef = nil
        }
    }

    fileprivate func fired() {
        let now = Date()
        guard now.timeIntervalSince(lastFired) > 0.5 else { return }
        lastFired = now
        let callback = onFire
        Task { @MainActor in callback() }
    }

    private func installHandlerIfNeeded() {
        guard eventHandler == nil else { return }
        var spec = EventTypeSpec(
            eventClass: OSType(kEventClassKeyboard),
            eventKind: UInt32(kEventHotKeyPressed)
        )
        let userData = Unmanaged.passUnretained(self).toOpaque()
        InstallEventHandler(
            GetApplicationEventTarget(),
            hotKeyEventHandler,
            1, &spec, userData, &eventHandler
        )
    }

    static let signature: OSType = {
        let chars = "RWND".utf8.prefix(4)
        return chars.reduce(0) { ($0 << 8) + OSType($1) }
    }()

    struct Combo { let keyCode: UInt32; let modifiers: UInt32 }

    static func parse(_ raw: String) -> Combo? {
        var modifiers: UInt32 = 0
        var keyToken: String?
        for part in raw.split(separator: "+") {
            let token = part.trimmingCharacters(in: .whitespaces)
            if let modifier = HotkeyModifier.from(alias: token) {
                modifiers |= modifier.carbonFlag
            } else {
                keyToken = token
            }
        }
        guard let keyToken, let keyCode = keyCode(for: keyToken) else { return nil }
        return Combo(keyCode: keyCode, modifiers: modifiers)
    }

    static func keyCode(for token: String) -> UInt32? {
        if token.count == 1, let scalar = token.uppercased().unicodeScalars.first {
            if let code = letterCodes[Character(scalar)] { return code }
            if let code = digitCodes[Character(scalar)] { return code }
        }
        switch token.lowercased() {
        case "space": return UInt32(kVK_Space)
        case "return", "enter": return UInt32(kVK_Return)
        case "tab": return UInt32(kVK_Tab)
        case "escape", "esc": return UInt32(kVK_Escape)
        default: return nil
        }
    }

    private static let letterCodes: [Character: UInt32] = [
        "A": UInt32(kVK_ANSI_A), "B": UInt32(kVK_ANSI_B), "C": UInt32(kVK_ANSI_C),
        "D": UInt32(kVK_ANSI_D), "E": UInt32(kVK_ANSI_E), "F": UInt32(kVK_ANSI_F),
        "G": UInt32(kVK_ANSI_G), "H": UInt32(kVK_ANSI_H), "I": UInt32(kVK_ANSI_I),
        "J": UInt32(kVK_ANSI_J), "K": UInt32(kVK_ANSI_K), "L": UInt32(kVK_ANSI_L),
        "M": UInt32(kVK_ANSI_M), "N": UInt32(kVK_ANSI_N), "O": UInt32(kVK_ANSI_O),
        "P": UInt32(kVK_ANSI_P), "Q": UInt32(kVK_ANSI_Q), "R": UInt32(kVK_ANSI_R),
        "S": UInt32(kVK_ANSI_S), "T": UInt32(kVK_ANSI_T), "U": UInt32(kVK_ANSI_U),
        "V": UInt32(kVK_ANSI_V), "W": UInt32(kVK_ANSI_W), "X": UInt32(kVK_ANSI_X),
        "Y": UInt32(kVK_ANSI_Y), "Z": UInt32(kVK_ANSI_Z),
    ]

    private static let digitCodes: [Character: UInt32] = [
        "0": UInt32(kVK_ANSI_0), "1": UInt32(kVK_ANSI_1), "2": UInt32(kVK_ANSI_2),
        "3": UInt32(kVK_ANSI_3), "4": UInt32(kVK_ANSI_4), "5": UInt32(kVK_ANSI_5),
        "6": UInt32(kVK_ANSI_6), "7": UInt32(kVK_ANSI_7), "8": UInt32(kVK_ANSI_8),
        "9": UInt32(kVK_ANSI_9),
    ]
}

private func hotKeyEventHandler(
    _ next: EventHandlerCallRef?, _ event: EventRef?, _ userData: UnsafeMutableRawPointer?
) -> OSStatus {
    guard let userData else { return noErr }
    let manager = Unmanaged<HotkeyManager>.fromOpaque(userData).takeUnretainedValue()
    manager.fired()
    return noErr
}
