# Rewinder (native SwiftUI + Rust engine)

Rewinder is a macOS rolling-replay recorder. The UI is a native SwiftUI app that
adopts Apple's **Liquid Glass** material; it drives a shared Rust capture engine
linked in as a C-ABI static library. (The previous Tauri + React shell has been
removed — the Rust engine crate lives on under `src-tauri/`.)

```
SwiftUI glass UI (RewinderApp)  ──C ABI FFI──►  ffi.rs ──►  Engine (Rust)
                                                              │
                                                   sck_capture helper + ffmpeg
```

## Run as a desktop app (development)

The Swift package links `src-tauri/target/debug/librewinder_lib.a`, so build the
Rust lib first, then run from inside `RewinderApp/`:

```bash
cd src-tauri && cargo build --lib && cd ..
cd RewinderApp && swift run
```

## Build a packaged `.app`

```bash
cd RewinderApp
scripts/package_app.sh            # release (default) -> RewinderApp/build/Rewinder.app
scripts/package_app.sh --debug    # faster debug build
```

This builds the static lib + Swift binary, bundles
`Contents/Resources/bin/{rewinder-sck-capture,ffmpeg,ffprobe}`, and ad-hoc signs
with `Resources/Rewinder.entitlements`. See [`RewinderApp/README.md`](RewinderApp/README.md)
for architecture, prerequisites, and Developer-ID signing / notarization steps.

## Notes

- Global hotkey default: `Ctrl+Option+R`
- Live capture is armed whenever replay is enabled; the rolling buffer holds the
  last N seconds (configurable in Settings).
- Closing the window keeps Rewinder armed in the menu bar; use the tray
  `Quit Rewinder` to exit fully (which stops capture and all helper processes).
- Replay clips are saved to the configured output directory.
- If no loopback audio device (BlackHole/Loopback/Soundflower) is present, capture
  runs video-only.
- Lean rust-analyzer defaults for the engine crate live in
  [`.vscode/settings.json`](.vscode/settings.json).

## Troubleshooting: Stale macOS Capture Indicator

If macOS still shows a capture indicator after you stopped replay, run this
operator cleanup:

```bash
pkill -f "rewinder-sck-capture|ffmpeg.*\.rewinder-live|ffmpeg.*video\.pipe|ffmpeg.*system_audio\.pipe|ffmpeg.*mic_audio\.pipe"
killall ControlCenter
killall SystemUIServer
```

Then reopen Control Center and verify the capture tile is gone.

If the indicator still appears after Rewinder is fully exited and no capture
workers remain, treat that as macOS UI state and reset Control Center or sign
out/in rather than changing Rewinder behavior again.
