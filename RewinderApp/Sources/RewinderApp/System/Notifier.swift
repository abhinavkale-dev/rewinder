import AppKit
import UserNotifications

@MainActor
enum Notifier {
    static let isBundled = Bundle.main.bundleIdentifier != nil

    static func requestAuthorizationIfBundled() {
        guard isBundled else { return }
        UNUserNotificationCenter.current()
            .requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func post(title: String, body: String, sound: Bool = true) {
        if isBundled {
            let content = UNMutableNotificationContent()
            content.title = title
            content.body = body
            if sound { content.sound = .default }
            if let logo = logoAttachment() {
                content.attachments = [logo]
            }
            let request = UNNotificationRequest(
                identifier: UUID().uuidString, content: content, trigger: nil)
            UNUserNotificationCenter.current().add(request)
        } else {
            let script =
                "display notification \"\(escaped(body))\" with title \"\(escaped(title))\""
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
            process.arguments = ["-e", script]
            try? process.run()
        }
    }

    static func playCue(_ name: String) {
        NSSound(named: NSSound.Name(name))?.play()
    }

    private static var bundledCues: [String: NSSound] = [:]

    static func playCue(bundled name: String, fallback: String? = nil) {
        if let cached = bundledCues[name] {
            cached.stop()
            cached.play()
            return
        }
        if let url = Bundle.main.url(forResource: name, withExtension: "wav", subdirectory: "Sounds"),
           let sound = NSSound(contentsOf: url, byReference: true) {
            bundledCues[name] = sound
            sound.play()
        } else if let fallback {
            playCue(fallback)
        }
    }

    private static func escaped(_ text: String) -> String {
        text.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
    }

    private static func logoAttachment() -> UNNotificationAttachment? {
        guard let image = AppIcon.image(), let png = pngData(from: image) else { return nil }
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("rewinder-logo-\(UUID().uuidString).png")
        do {
            try png.write(to: url)
            return try UNNotificationAttachment(identifier: "appLogo", url: url, options: nil)
        } catch {
            return nil
        }
    }

    private static func pngData(from image: NSImage) -> Data? {
        if let best = image.representations
            .compactMap({ $0 as? NSBitmapImageRep })
            .max(by: { $0.pixelsWide < $1.pixelsWide }) {
            return best.representation(using: .png, properties: [:])
        }
        guard let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff) else { return nil }
        return rep.representation(using: .png, properties: [:])
    }
}
