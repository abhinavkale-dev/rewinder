# Rewinder (Tauri + React)

## Run as desktop app

```bash
npm install
npm run desktop:dev
```

This launches the native Tauri window (not a browser tab).

## Build desktop app

```bash
npm run desktop:build
```

## Notes

- Global hotkey default: `Ctrl+Option+R`
- Live capture is always armed when replay is enabled.
- Replay clips are saved to the configured output directory.
- In `tauri dev`, closing the window exits Rewinder fully; the dev build does not keep a tray/background session alive.
- Packaged Rewinder builds keep the tray/background flow when replay is armed; use tray `Quit Rewinder` to exit fully.
- If no loopback audio device (BlackHole/Loopback/Soundflower) is present, capture runs video-only.
- In `tauri dev` launched from Cursor/Terminal, macOS may label the system capture indicator with that host app; packaged Rewinder builds reflect user-facing app identity.

## Troubleshooting: Stale macOS Capture Indicator

If macOS still shows a capture indicator after you stopped replay, run this operator cleanup:

```bash
pkill -f "rewinder-sck-capture|ffmpeg.*\\.rewinder-live|ffmpeg.*video\\.pipe|ffmpeg.*system_audio\\.pipe|ffmpeg.*mic_audio\\.pipe"
pkill -f "/Applications/Cursor\\.app"
killall ControlCenter
killall SystemUIServer
```

Then reopen Control Center and verify the capture tile is gone.

If the indicator still appears after Rewinder is fully exited and no capture workers remain, treat that as macOS UI state and reset Control Center or sign out/in rather than changing Rewinder behavior again.

## Performance

- Lean rust-analyzer defaults are committed in `/Users/apple/Desktop/rewinder/.vscode/settings.json`.
- Perf profile and override guidance: `/Users/apple/Desktop/rewinder/docs/perf.md`.
- OSS behavior reference map: `/Users/apple/Desktop/rewinder/docs/oss-reference-map.md`.
- Capture a perf baseline snapshot:

```bash
./scripts/perf/smoke.sh
```
