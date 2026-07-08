import SwiftUI

struct HomeModel {
    let headline: String
    let subtext: String
    let tone: Tone
    let phase: HomePhase
    let replayEnabled: Bool
    let replayTarget: Double
    let captureLive: Bool
    let bufferFull: Bool
    let saveDisabled: Bool
    let alerts: [HomeAlert]
    let screenStatus: PermissionStatusIcon.Status
    let micStatus: PermissionStatusIcon.Status

    init(state: EngineState, displayFill: Double, rebuildNote: String? = nil) {
        let s = state.settings
        replayEnabled = s.replayEnabled
        replayTarget = s.replayDurationSecs > 0 ? Double(s.replayDurationSecs) : state.replayTargetSecs
        captureLive = state.captureHealth == "running" || state.captureHealth == "degraded"
        let projected = min(max(displayFill.rounded(), 0), replayTarget)
        let bufferFull = captureLive && projected >= replayTarget
        self.bufferFull = bufferFull
        let fillSecs = projected
        let screenGranted = state.permission.screenRecordingGranted
        saveDisabled = state.isSaving || state.pendingSave

        var built: [HomeAlert] = []
        let code = state.armBlockerCode

        if code == "capture_paused" {
            built.append(HomeAlert(
                id: "resume", tone: .warning, icon: "waveform.path.ecg",
                title: "Capture paused",
                message: "Capture is paused. Resume to keep your replay protected.",
                actionTitle: "Resume Capture", kind: .resume))
        }
        if code == "user_stopped_sharing", !s.replayEnabled {
            built.append(HomeAlert(
                id: "restart", tone: .warning, icon: "waveform.path.ecg",
                title: "Capture stopped",
                message: "Screen sharing was stopped. Restart capture to re-arm.",
                actionTitle: "Restart Capture", kind: .restart))
        }
        let showDownloads = code == "output_dir_permission_required" || !state.permission.outputDirWritable
        if showDownloads {
            built.append(HomeAlert(
                id: "downloads", tone: .danger, icon: "folder",
                title: "Downloads permission needed",
                message: "Rewinder can't write clips to the output folder yet.",
                actionTitle: "Enable Downloads", kind: .downloads))
        }
        alerts = built

        screenStatus = screenGranted ? .granted : .needed
        let micBlocked = ["denied", "restricted", "not_determined"].contains(state.micPermissionStatus)
        micStatus = !s.micEnabled ? .off : (micBlocked ? .needed : .granted)

        let blocking = !screenGranted || !built.isEmpty
        let target = Int(replayTarget)
        if state.isSaving {
            headline = "Saving your replay"
            subtext = "Writing the last \(target)s to a clip."
            tone = .accent
            phase = .saving
        } else if blocking {
            headline = "Action needed"
            subtext = "Resolve the highlighted item below to keep your replay protected."
            tone = .warning
            phase = .permission
        } else if s.replayEnabled, screenGranted, bufferFull {
            headline = "You're protected"
            subtext = "Rewinder is holding the last \(target)s, ready to save instantly."
            tone = .success
            phase = .protected
        } else if s.replayEnabled, screenGranted, captureLive {
            if let rebuildNote {
                headline = "Rebuilding buffer"
                subtext = "\(rebuildNote) · \(Int(fillSecs))s of \(target)s…"
            } else {
                headline = "Building your buffer"
                subtext = "Holding \(Int(fillSecs))s of \(target)s…"
            }
            tone = .accent
            phase = .building
        } else if s.replayEnabled, screenGranted {
            let startup = describeCaptureStartup(captureHealth: state.captureHealth, phase: state.captureStartPhase)
            headline = startup.headline
            subtext = startup.sub
            tone = .accent
            phase = .starting
        } else if s.replayEnabled {
            headline = "Starting up"
            subtext = "Getting the capture ready…"
            tone = .accent
            phase = .starting
        } else {
            headline = "Replay is off"
            subtext = "Tap the button to keep the last \(target)s of your screen."
            tone = .neutral
            phase = .off
        }
    }
}
