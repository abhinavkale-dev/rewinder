import AppKit

@MainActor
enum AppIcon {
    static func image() -> NSImage? {
        loadResource(named: "AppIcon", file: "AppIcon.png")
    }

    static func trayImage() -> NSImage? {
        guard let img = loadResource(named: "TrayIcon", file: "TrayIcon.png") else { return nil }
        let height: CGFloat = 15
        let width = img.size.height > 0 ? height * img.size.width / img.size.height : height
        img.size = NSSize(width: width, height: height)
        img.isTemplate = false
        return img
    }

    private static func loadResource(named name: String, file: String) -> NSImage? {
        if let bundled = Bundle.main.image(forResource: name) {
            return bundled
        }
        let source = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Resources/\(file)")
        return NSImage(contentsOf: source)
    }
}
