# Rewinder — Performance Report (native SwiftUI build) & ShadowPlay Comparison

**Date:** 2026‑07‑10  ·  **Build:** release 0.1.0 (`scripts/package_app.sh`, signed "Rewinder Dev")
**Test machine:** Apple Silicon macOS (darwin 25.4.0), **64 GB RAM** → boots **1080p60** defaults
**Test conditions:** machine heavily loaded during measurement (Cursor IDE, dev servers, measurement shells, plus Rewinder itself capturing the live screen at 60 fps with mic + RNNoise noise removal active). Treat capture‑stack CPU as an upper bound.

> TL;DR — The Tauri/WebView stack is **gone**. Rewinder is now a native SwiftUI app with the Rust engine statically linked in‑process (FFI), so the whole UI + engine costs **~33 MB** of real memory — the old build's WebView alone was ~105 MB. Total armed footprint at 1080p60 is **~150–260 MB** physical (dominated by ScreenCaptureKit's IOSurface pools, which vary with screen content), disk write is **~0.4–1.3 MB/s**, and the app process idles at ~0–1 % CPU. The old report's #1 gap — **no battery awareness** — is fixed: a battery guard now samples the power source and caps fps on battery (default 30), restoring quality on AC. Remaining structural inefficiency: the separate `ffmpeg` process for encode/mux/audio‑DSP.

---

## 1. What changed since the last report (2026‑06‑08)

The architecture the June report measured no longer exists:

| | June (Tauri) | Now (native) |
|---|---|---|
| UI | WKWebView (3 WebKit helper processes, ~105 MB phys) | **SwiftUI in‑process** (0 extra processes, 0 extra MB) |
| Engine ↔ UI | Tauri IPC to a `rewinder` host process | **Rust static library linked into the app via C FFI** (`src-tauri/src/ffi.rs`, `CRewinderFFI`) |
| Processes armed | 6 (host + 3×WebKit + helper + ffmpeg) | **3** (app + `rewinder-sck-capture` + `ffmpeg`) |
| Main binary | 9.27 MB host + 238 KB JS bundle | **4.8 MB** single executable (app bundle 94 MB, ~90 MB of which is the vendored static ffmpeg/ffprobe pair) |
| "Path A" WebView unload | needed (reclaim ~105 MB on close‑to‑tray) | **moot — there is no WebView to unload** |
| Battery awareness | none (confirmed gap) | **battery guard shipped** (see §4) |
| Mic audio | +10 dB gain, basic mix | **RNNoise AI denoise (`arnndn`, bd.rnnn model) + +6 dB gain**, `volume` filter on system audio |

---

## 2. Measured results — per‑process breakdown

Memory is reported two ways: `ps` RSS **overcounts** shared frameworks and mapped IOSurface/GPU memory; `vmmap` **Physical footprint** is what Activity Monitor calls "Memory" and is the honest number.

### 2a. ARMED, 1080p60, window open, mic + RNNoise active

| Process | Role | `ps` RSS | `vmmap` phys ("Memory") | CPU (1 core) |
|---|---|---:|---:|---:|
| `Rewinder` (app) | SwiftUI UI + Rust engine (FFI, in‑process) | 155 MB | **33 MB** (peak 73) | ~0.3–1.4 % |
| `rewinder-sck-capture` | ScreenCaptureKit helper (video + system audio + mic) | 237 MB | **~100–204 MB** (peak 309) | ~24–26 % |
| `ffmpeg` | HW H.264 encode + RNNoise + amix + segment mux | 184 MB | **17–19 MB** (peak 57) | ~26–35 % |

**Subtotals (honest phys footprint):**
- UI + engine: **~33 MB** ← was ~131 MB (host + WebView) in June
- Capture stack (2 procs): **~120–225 MB**, dominated by the SCK helper's IOSurface frame pools (varies with screen content and settles after startup; two live samples on this session read 99 MB and 204 MB)
- **Total armed ≈ 150–260 MB phys** (June: ~268 MB, and that was *without* today's RNNoise/mic DSP)

Backgrounding the window no longer changes the footprint materially — there's no WebView to tear down; the SwiftUI window is a few MB inside the app process.

### 2b. CPU notes

- The **app process** is event‑driven: ~0–1.4 % while armed with the window open (the Home ring animation), ~0 % in the tray.
- **Encode is HW‑offloaded** (`h264_videotoolbox -realtime 1`); ffmpeg's ~26–35 % of one core is mostly **software audio DSP** — RNNoise (`arnndn`) inference on the mic, `amix`, `volume` filters — plus mux/FIFO I/O. Disabling "Reduce mic background noise" or running system‑audio‑only drops this substantially.
- The helper's ~25 % of one core is 60 fps NV12 frame delivery under a heavily loaded machine — upper bound, not steady‑state on a quiet system.

### 2c. Disk (rolling buffer)

- Live buffer at `<output dir>/.rewinder-live`: **265 × 0.5 s MP4 segments ≈ 132 s retained, 47 MB on disk ≈ 0.36 MB/s** for a mostly‑static desktop.
- Bitrate is capped by the adaptive ladder (`-maxrate` observed live at 5500k after a step‑down under load; 1080p60 full quality caps at 10 Mbps ≈ 1.27 MB/s).
- The buffer is **encoded segments on disk, not frames in RAM** → buffer length costs disk, not memory. Saving = segment concat/stream‑copy (`instant_mp4`), no re‑encode.

---

## 3. Adaptive performance guard — still bidirectional

Observed live this session: boot at 1080p60 @ 10 Mbps → stepped down under sustained load (running at `-maxrate 5500k` during measurement) → recovers when pressure clears. The recovery worker samples capture/playback speed, output fps, frame drops, queue overflows, capture‑stack CPU and RSS growth (soft/hard), macOS memory pressure, and thermal state (`pmset -g therm`), stepping `runtime_profile_index` along **1080p60 → 1080p30 → 1080p30 (lower bitrate) → 720p30** and back. Reason codes are surfaced to the UI (`state_projection.rs`), so step‑downs are user‑visible now.

## 4. Battery guard — the June gap, now closed

June's report flagged: *"the guard reacts to CPU/RSS/thermal/memory but never to power source or battery level."* That is no longer true:

- `process_metrics.rs` samples the power source via `pmset -g batt` (`"battery"` / `"ac"`).
- `profile.rs` computes a **battery floor** (`battery_floor_index`): on battery, with `battery_guard_enabled` (default **true**), quality is capped at `battery_max_fps` (default **30**, validated 10–120).
- `recovery.rs` handles `CaptureRestartReason::PowerSourceChanged` both ways: unplugging **caps quality in one step** (reason code `on_battery`); plugging back into AC **restores the requested quality in one restart**, not one rung per cycle. An AC plug‑in deliberately does *not* jump quality back up while a genuine overload is what's binding the index.
- Settings expose `battery_guard_enabled` / `battery_max_fps` (patchable, validated, covered by tests in `settings/mod.rs`).

What it still does **not** do: react to battery *percentage* (only source), or measure Watts (needs `sudo powermetrics`).

