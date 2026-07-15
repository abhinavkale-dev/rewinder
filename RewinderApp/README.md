# RewinderApp (native SwiftUI)

A native macOS SwiftUI front-end for the Rewinder engine, adopting Apple's
**Liquid Glass** material (macOS 26+). It reuses the existing Rust engine and the
`sck_capture` Swift helper unchanged by linking the engine as a C-ABI static
library and reimplementing the window, menu-bar tray, and global hotkey natively.

## Architecture

```
SwiftUI views (Liquid Glass)
        │  @Observable
   RewinderEngine  ──C calls (JSON)──►  rewinder_ffi.h
        ▲                                     │
        └──── event callback (JSON) ◄─── librewinder_lib.a (Rust engine)
                                              │
                                   sck_capture + ffmpeg (spawned)
```

- **FFI bridge** — `Sources/CRewinderFFI/include/rewinder_ffi.h` mirrors the
  `#[no_mangle]` surface in `../src-tauri/src/ffi.rs` (13 commands + event
  callback + init/shutdown). Each command returns a JSON envelope
  (`{ ok, data }` / `{ ok: false, error }`).
- **RewinderEngine** (`Sources/RewinderApp/RewinderEngine.swift`) — `@Observable`
  `@MainActor` store; decodes DTOs, marshals the C event callback onto the main
  actor, exposes `engineState` / `settings` / `clips` / `microphones`.
- **Views** — `HomeView` (Liquid Glass power button = Replay toggle, Save,
  status, permission alert cards, info cards), `SettingsView` (simplified
  Recording/Audio/Saving/Shortcuts groups + Advanced + Troubleshooting),
  `ClipsView`, and `LoadingView` (gradient boot splash).
- **Native shell** — `AppDelegate` provides the `NSStatusItem` tray (Save /
  Resolution / Replay Duration / Audio / Settings / Quit with dynamic labels),
  a Carbon `RegisterEventHotKey` global hotkey (`HotkeyManager`) with primary +
  fallback registration, close-to-tray `.accessory` lifecycle, and a flock-based
  single-instance guard (`SingleInstance`).

## Prerequisites

- macOS 26 (Tahoe) or newer, Xcode 26 toolchain (Swift 6.2+).
- The Rust toolchain (for `cargo`).
- `ffmpeg` / `ffprobe` available on `PATH` (bundled automatically at package time).

## Build & run (development)

The Swift package links `../src-tauri/target/debug/librewinder_lib.a`, so build
the Rust lib first, then run from **inside this directory**:

```bash
cd ../src-tauri && cargo build --lib && cd -
swift run            # or: swift build && ./.build/debug/RewinderApp
```

In an unbundled dev run the engine uses its compile-time helper path; `ffmpeg`
falls back to `PATH` / Homebrew.

## Package a .app

```bash
scripts/package_app.sh            # release (default)
scripts/package_app.sh --debug    # faster debug build
```

This builds the release static lib + Swift binary, assembles
`build/Rewinder.app` with `Contents/Resources/bin/{rewinder-sck-capture,ffmpeg,ffprobe}`,
copies `Resources/Info.plist`, and **ad-hoc** signs with `Resources/Rewinder.entitlements`.
At launch `BundleResources` sets `REWINDER_SCK_HELPER_BIN` to the bundled helper
(`ffmpeg` auto-resolves from `Resources/bin`).

If `ffmpeg` isn't on `PATH`, set `REWINDER_FFMPEG_DIR` to a folder containing
`ffmpeg`/`ffprobe` before running the script, or drop them into
`Contents/Resources/bin` afterward.

## Distribute a .dmg

The full release pipeline (Developer ID signing, notarization, stapling, dmg)
is automated:

```bash
scripts/release_dmg.sh          # -> dist/Rewinder-<version>.dmg (signed + notarized + stapled)
```

Supporting scripts:
- `scripts/fetch_ffmpeg.sh` — downloads a self-contained **static** arm64
  ffmpeg/ffprobe into `vendor/ffmpeg` (so the app doesn't depend on Homebrew).
- `scripts/verify_ffmpeg.sh` — fails the build if ffmpeg links any non-system
  library or is missing a required encoder/filter.

Dry run without an Apple Developer cert:
```bash
REWINDER_SKIP_NOTARIZE=1 REWINDER_CODESIGN_IDENTITY="-" scripts/release_dmg.sh
```

> The previous Tauri/React shell (`src/`, root `package.json`, `tauri.conf.json`,
> Tauri command/tray wiring) has been removed; this SwiftUI app is the only
> frontend. The Rust engine crate is shared and still lives under `src-tauri/`.
