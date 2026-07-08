import SwiftUI

enum AlertKind {
    case resume, restart, screen, downloads, mic, micDenied
}

struct HomeAlert: Identifiable {
    let id: String
    let tone: Tone
    let icon: String
    let title: String
    let message: String
    let actionTitle: String
    let kind: AlertKind
}

func describeCaptureStartup(captureHealth: String, phase: String?) -> (headline: String, sub: String) {
    let restarting = captureHealth == "restarting"
    switch phase {
    case "helper_spawned":
        return ("Waking the recorder", "Step 1 of 6 · Spinning up the screen-capture engine…")
    case "stream_start_requested":
        return ("Connecting your display", "Step 2 of 6 · Linking up to your screen…")
    case "stream_started":
        return ("Capture is live", "Step 3 of 6 · Waiting for the first frames…")
    case "first_video_frame":
        return ("Recording video", "Step 4 of 6 · Capturing your first frames…")
    case "first_audio_frame":
        return ("Syncing audio", "Step 5 of 6 · Lining your audio up with the video…")
    case "first_segment":
        return ("Building your buffer", "Step 6 of 6 · Finishing the first clip…")
    default:
        return restarting
            ? ("Restarting capture", "Getting the capture pipeline ready…")
            : ("Starting replay", "Launching the screen-capture engine…")
    }
}
