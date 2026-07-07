import AppKit
import ObjectiveC
import SwiftUI
import UserNotifications

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate,
    UNUserNotificationCenterDelegate
{
    private var engine: RewinderEngine?
    private var statusItem: NSStatusItem?
    private var hotkeys: HotkeyManager?
    private var mainWindow: NSWindow?
    private var configured = false

    private var primaryItem: NSMenuItem?
    private var statusLineItem: NSMenuItem?
    private var resolutionItems: [Int: NSMenuItem] = [:]
    private var durationItems: [Int: NSMenuItem] = [:]
    private var displayItems: [String: NSMenuItem] = [:]
    private var audioSystemItem: NSMenuItem?
    private var audioMicItem: NSMenuItem?
    private var registeredHotkeySignature: String?

    private let resolutionPresets = Array(Presets.resolutions.reversed())
    private let durationPresets = Presets.durations

    private lazy var appIcon = AppIcon.image()
    private lazy var trayIcon = AppIcon.trayImage()

    private var hasSeenCaptureLive = false
    private var lastNotifiedAttentionCode: String?

    private var attentionActive = false
    private var activeAttention: (code: String, title: String, body: String)?
    private var reNotifyTimer: Timer?
    private let reNotifyInterval: TimeInterval = 300

    func applicationDidFinishLaunching(_ notification: Notification) {
        CrashGuard.install()
        NSMenuItem.suppressAutomaticImages()
        applyDockIcon()
        Notifier.requestAuthorizationIfBundled()
        if Notifier.isBundled {
            UNUserNotificationCenter.current().delegate = self
        }
        NSApp.setActivationPolicy(.accessory)
        let primary = SingleInstance.acquire { [weak self] in self?.showWindow() }
        if !primary {
            NSApp.terminate(nil)
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        false
    }

    func applicationWillTerminate(_ notification: Notification) {
        engine?.shutdown()
    }

    func attach(_ engine: RewinderEngine) {
        self.engine = engine
        guard !configured else { return }
        configured = true
        setupStatusItem()
        setupHotkeys()
        setupWindow()
        CrashGuard.updatePaths(outputDir: engine.settings?.outputDir)
        engine.onStateChange = { [weak self] in
            self?.updateTrayLabels()
            self?.reconcileHotkeys()
            CrashGuard.updatePaths(outputDir: self?.engine?.settings?.outputDir)
        }
        engine.onClipSaved = { [weak self] in
            self?.winkTray()
        }
        updateTrayLabels()
    }

    private func winkTray() {
        guard !NSWorkspace.shared.accessibilityDisplayShouldReduceMotion,
              let button = statusItem?.button else { return }
        NSAnimationContext.runAnimationGroup({ ctx in
            ctx.duration = 0.14
            button.animator().alphaValue = 0.3
        }, completionHandler: {
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = 0.3
                button.animator().alphaValue = 1.0
            }
        })
    }

    private func setupStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            button.action = #selector(statusItemClicked)
            button.target = self
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
        }
        statusItem = item
        updateStatusIcon(attention: false)
    }

    private func updateStatusIcon(attention: Bool) {
        guard let button = statusItem?.button else { return }
        if attention {
            let image = NSImage(systemSymbolName: "exclamationmark.triangle.fill", accessibilityDescription: "Rewinder: recording off")
            image?.isTemplate = true
            button.image = image
            button.contentTintColor = .systemOrange
            button.imagePosition = .imageLeading
            button.attributedTitle = NSAttributedString(
                string: " Off",
                attributes: [
                    .foregroundColor: NSColor.systemOrange,
                    .font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize, weight: .semibold),
                ]
            )
        } else {
            button.attributedTitle = NSAttributedString(string: "")
            button.imagePosition = .imageOnly
            if let owl = trayIcon {
                button.image = owl
                button.contentTintColor = nil
            } else {
                let fallback = NSImage(systemSymbolName: "backward.end.circle", accessibilityDescription: "Rewinder")
                fallback?.isTemplate = true
                button.image = fallback
                button.contentTintColor = nil
            }
        }
    }

    private func buildMenu() -> NSMenu {
        let menu = NSMenu()

        let open = NSMenuItem(title: "Open Rewinder", action: #selector(openMainWindow), keyEquivalent: "")
        open.target = self
        menu.addItem(open)
        menu.addItem(.separator())

        let primary = NSMenuItem(title: "Save Replay", action: #selector(primaryAction), keyEquivalent: "")
        primary.target = self
        menu.addItem(primary)
        primaryItem = primary

        menu.addItem(.separator())

        let status = NSMenuItem(title: "Starting…", action: nil, keyEquivalent: "")
        status.isEnabled = false
        menu.addItem(status)
        statusLineItem = status

        menu.addItem(.separator())

        let resMenu = NSMenu()
        for height in resolutionPresets {
            let mi = NSMenuItem(title: "\(height)p", action: #selector(selectResolution(_:)), keyEquivalent: "")
            mi.target = self
            mi.tag = height
            resMenu.addItem(mi)
            resolutionItems[height] = mi
        }
        let resItem = NSMenuItem(title: "Resolution", action: nil, keyEquivalent: "")
        resItem.submenu = resMenu
        menu.addItem(resItem)

        displayItems.removeAll()
        let displays = DisplayDevice.connected()
        if displays.count > 1 {
            let dispMenu = NSMenu()
            for display in displays {
                let mi = NSMenuItem(title: display.name, action: #selector(selectDisplay(_:)), keyEquivalent: "")
                mi.target = self
                mi.representedObject = display.id
                dispMenu.addItem(mi)
                displayItems[display.id] = mi
            }
            let dispItem = NSMenuItem(title: "Display", action: nil, keyEquivalent: "")
            dispItem.submenu = dispMenu
            menu.addItem(dispItem)
        }

        let durMenu = NSMenu()
        for secs in durationPresets {
            let title = secs >= 60 ? (secs == 60 ? "60 seconds" : "\(secs / 60) minutes") : "\(secs) seconds"
            let mi = NSMenuItem(title: title, action: #selector(selectDuration(_:)), keyEquivalent: "")
            mi.target = self
            mi.tag = secs
            durMenu.addItem(mi)
            durationItems[secs] = mi
        }
        let durItem = NSMenuItem(title: "Replay Duration", action: nil, keyEquivalent: "")
        durItem.submenu = durMenu
        menu.addItem(durItem)

        let audioMenu = NSMenu()
        let sys = NSMenuItem(title: "System Audio Only", action: #selector(selectAudioSystem), keyEquivalent: "")
        sys.target = self
        audioMenu.addItem(sys)
        audioSystemItem = sys
        let mic = NSMenuItem(title: "System Audio + Mic", action: #selector(selectAudioMic), keyEquivalent: "")
        mic.target = self
        audioMenu.addItem(mic)
        audioMicItem = mic
        let audioItem = NSMenuItem(title: "Audio", action: nil, keyEquivalent: "")
        audioItem.submenu = audioMenu
        menu.addItem(audioItem)

        menu.addItem(.separator())

        let settings = NSMenuItem(title: "Settings…", action: #selector(openSettings), keyEquivalent: "")
        settings.target = self
        menu.addItem(settings)

        let quit = NSMenuItem(title: "Quit Rewinder", action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)

        return menu
    }

    private func updateTrayLabels() {
        guard let state = engine?.engineState else { return }
        let settings = state.settings

        switch state.lifecycleState {
        case "armed", "saving_replay":
            primaryItem?.title = "Save Replay"
            primaryItem?.isEnabled = true
        case "permission_required":
            primaryItem?.title = "Grant Permission"
            primaryItem?.isEnabled = true
        case "booting":
            primaryItem?.title = "Starting…"
            primaryItem?.isEnabled = false
        case "disabled":
            primaryItem?.isEnabled = true
            primaryItem?.title = state.armBlockerCode == "capture_paused" ? "Resume Capture" : "Restart Capture"
        default:
            primaryItem?.title = "Save Replay"
            primaryItem?.isEnabled = true
        }

        switch state.lifecycleState {
        case "armed", "saving_replay":
            statusLineItem?.title = "Recording · \(settings.replayDurationSecs)s · \(settings.videoResolution)p \(settings.fps)fps"
        case "disabled":
            statusLineItem?.title = "Paused"
        case "permission_required":
            statusLineItem?.title = "Permission needed"
        default:
            statusLineItem?.title = "Starting…"
        }

        for (height, item) in resolutionItems {
            item.state = (height == settings.videoResolution) ? .on : .off
        }
        for (secs, item) in durationItems {
            item.state = (secs == settings.replayDurationSecs) ? .on : .off
        }
        if !displayItems.isEmpty {
            let displays = DisplayDevice.connected()
            for (id, item) in displayItems {
                let display = displays.first { $0.id == id }
