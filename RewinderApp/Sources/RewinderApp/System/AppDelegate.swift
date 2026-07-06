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
                item.state = display?.isEffectiveSelection(
                    storedId: settings.selectedDisplayId, in: displays
                ) == true ? .on : .off
            }
        }
        let isMicMode = settings.audioMode == "system_plus_mic" && settings.micEnabled
        audioSystemItem?.state = isMicMode ? .off : .on
        audioMicItem?.state = isMicMode ? .on : .off

        updateAttentionState(state)
    }

    private func updateAttentionState(_ state: EngineState) {
        if state.captureHealth == "running" || state.captureHealth == "degraded" {
            hasSeenCaptureLive = true
        }
        let attention = captureAttention(for: state)
        attentionActive = attention != nil
        activeAttention = attention
        updateStatusIcon(attention: attentionActive)
        updateDockBadge()
        guard let attention else {
            lastNotifiedAttentionCode = nil
            stopReNotifyTimer()
            return
        }
        guard hasSeenCaptureLive, attention.code != lastNotifiedAttentionCode else { return }
        lastNotifiedAttentionCode = attention.code
        Notifier.post(title: attention.title, body: attention.body)
        startReNotifyTimer()
    }

    private func updateDockBadge() {
        let minimized = mainWindow?.isMiniaturized ?? false
        NSApp.dockTile.badgeLabel = (attentionActive && minimized) ? "!" : nil
    }

    private var isWindowInForeground: Bool {
        guard let window = mainWindow else { return false }
        return NSApp.isActive && window.isVisible && !window.isMiniaturized
    }

    private func startReNotifyTimer() {
        stopReNotifyTimer()
        reNotifyTimer = Timer.scheduledTimer(
            withTimeInterval: reNotifyInterval, repeats: true
        ) { [weak self] _ in
            Task { @MainActor in self?.reNotifyIfStillOff() }
        }
    }

    private func stopReNotifyTimer() {
        reNotifyTimer?.invalidate()
        reNotifyTimer = nil
    }

    private func reNotifyIfStillOff() {
        guard attentionActive, let attention = activeAttention else {
            stopReNotifyTimer()
            return
        }
        guard !isWindowInForeground else { return }
        Notifier.post(title: attention.title, body: attention.body)
    }

    private func captureAttention(
        for state: EngineState
    ) -> (code: String, title: String, body: String)? {
        if state.armBlockerCode == "user_stopped_sharing" {
            return (
                "user_stopped_sharing", "Recording stopped",
                "Screen sharing was stopped. Open Rewinder to restart capture."
            )
        }
        if state.armBlockerCode == "capture_paused" {
            return (
                "capture_paused", "Recording paused",
                "Your replay buffer is not recording. Resume from the menu bar icon."
            )
        }
        if state.lifecycleState == "permission_required" {
            return (
                "permission_required", "Recording stopped",
                "Rewinder lost a required permission and can't record."
            )
        }
        if state.settings.replayEnabled, state.captureHealth == "stopped" {
            return (
                "capture_stopped", "Recording stopped",
                "The replay buffer stopped recording. Open Rewinder to restart it."
            )
        }
        if state.lifecycleState == "disabled",
           let code = state.armBlockerCode, code != "disabled" {
            return (
                code, "Recording stopped",
                "Rewinder isn't recording. Open it to restart capture."
            )
        }
        return nil
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping () -> Void
    ) {
        Task { @MainActor in
            self.engine?.pendingNavigation = .home
            self.showWindow()
        }
        completionHandler()
    }

    private func setupHotkeys() {
        let manager = HotkeyManager { [weak self] in
            self?.engine?.saveReplay(hotkey: true)
        }
        hotkeys = manager
        registerHotkeys()
    }

    private func registerHotkeys() {
        let primary = engine?.settings?.hotkey ?? "Ctrl+Option+R"
        let fallbacks = engine?.settings?.fallbackHotkeys ?? []
        let registration = hotkeys?.register(primary: primary, fallbacks: fallbacks)
        registeredHotkeySignature = hotkeySignature(primary: primary, fallbacks: fallbacks)
        if registration?.mode == HotkeyManager.Mode.none {
            Notifier.post(
                title: "Save hotkey unavailable",
                body: "“\(primary)” may conflict with another app. Pick a different shortcut in Settings."
            )
        }
    }

    private func reconcileHotkeys() {
        guard let settings = engine?.settings else { return }
        let signature = hotkeySignature(primary: settings.hotkey, fallbacks: settings.fallbackHotkeys)
        guard signature != registeredHotkeySignature else { return }
        registerHotkeys()
    }

    private func hotkeySignature(primary: String, fallbacks: [String]) -> String {
        ([primary] + fallbacks).joined(separator: "|")
    }

    private func setupWindow() {
        guard let window = NSApp.windows.first(where: { $0.canBecomeMain }) ?? NSApp.windows.first else { return }
        mainWindow = window
        window.delegate = self
        window.title = "Rewinder"
        window.isOpaque = true
        window.backgroundColor = Theme.appBackgroundNS
        window.titlebarAppearsTransparent = true
        showWindow()
    }

    private func showWindow() {
        guard let window = mainWindow ?? NSApp.windows.first else { return }
        mainWindow = window
        NSApp.setActivationPolicy(.regular)
        if window.isMiniaturized {
            window.deminiaturize(nil)
        } else {
            window.makeKeyAndOrderFront(nil)
        }
        NSApp.activate(ignoringOtherApps: true)
        applyDockIcon()
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showWindow()
        return true
    }

    private func applyDockIcon() {
        guard !Notifier.isBundled else { return }
        if let appIcon {
            NSApp.applicationIconImage = appIcon
        }
    }

    @objc private func openMainWindow() {
        engine?.pendingNavigation = .home
        showWindow()
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        NSApp.setActivationPolicy(.accessory)
        return false
    }

    func windowDidMiniaturize(_ notification: Notification) { updateDockBadge() }
    func windowDidDeminiaturize(_ notification: Notification) { updateDockBadge() }

    @objc private func statusItemClicked() {
        let menu = buildMenu()
        updateTrayLabels()
        statusItem?.menu = menu
        statusItem?.button?.performClick(nil)
        statusItem?.menu = nil
    }

    @objc private func primaryAction() {
        guard let engine, let state = engine.engineState else { return }
        switch state.lifecycleState {
        case "armed", "saving_replay":
            engine.saveReplay()
        case "permission_required":
            engine.grantScreenRecording()
        case "booting":
            break
        case "disabled":
            if state.armBlockerCode == "capture_paused" {
                engine.resumeCapture()
            } else {
                engine.setReplayEnabled(true)
            }
        default:
            engine.saveReplay()
        }
    }

    @objc private func selectResolution(_ sender: NSMenuItem) {
        engine?.applyPatch(["videoResolution": sender.tag])
    }

    @objc private func selectDuration(_ sender: NSMenuItem) {
        engine?.applyPatch(["replayDurationSecs": sender.tag])
    }

    @objc private func selectDisplay(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        engine?.applyPatch(["selectedDisplayId": id])
    }

    @objc private func selectAudioSystem() {
        engine?.applyPatch(["audioMode": "system_only", "micEnabled": false])
    }

    @objc private func selectAudioMic() {
        engine?.applyPatch(["audioMode": "system_plus_mic", "micEnabled": true])
    }

    @objc private func openSettings() {
        engine?.pendingNavigation = .settings
        showWindow()
    }

    @objc private func quit() {
        engine?.shutdown()
        NSApp.terminate(nil)
    }
}

private extension NSMenuItem {
    static func suppressAutomaticImages() {
        let original = #selector(getter: image)
        let replacement = #selector(rewinder_suppressedImage)
        guard let originalMethod = class_getInstanceMethod(NSMenuItem.self, original),
              let newMethod = class_getInstanceMethod(NSMenuItem.self, replacement) else { return }
        method_exchangeImplementations(originalMethod, newMethod)
    }

    @objc func rewinder_suppressedImage() -> NSImage? { nil }
}
