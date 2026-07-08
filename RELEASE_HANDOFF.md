# Rewinder — Release Handoff (build, sign, notarize, ship the DMG)

> **Audience:** an AI assistant (or human) on the release machine. Follow this
> top to bottom. The end product is a single file — `Rewinder-<version>.dmg`
> (plus its `.sha256`) — zipped and sent back to the repo owner.

## What this repo is

Rewinder is a native macOS "instant replay" app: a SwiftUI shell
(`RewinderApp/`) over a Rust capture engine (`src-tauri/`, built as a static
library, linked over C FFI). There is no Tauri/web runtime in the shipped app.
The release script builds everything, signs it, notarizes it, and wraps it in
a DMG.

## Goal

Run **one script** — `RewinderApp/scripts/release_dmg.sh` — to completion, then
zip and return the artifacts it writes to `RewinderApp/dist/`:

- `Rewinder-<version>.dmg` — signed, notarized, stapled
- `Rewinder-<version>.dmg.sha256`

## Machine requirements (check these first)

1. **Apple-Silicon Mac** (arm64 only; do not attempt on Intel).
2. **macOS 26 (Tahoe)** — the app targets `.macOS(.v26)`.
3. **Xcode 26** with command-line tools (Swift 6.2+): `swift --version`
4. **Rust toolchain**: `cargo --version` (install via rustup if missing).
5. Optional but recommended: `brew install create-dmg` — gives the DMG the
   designed window (background art + Applications drop target). Without it the
   script falls back to a plain `hdiutil` DMG that works but looks bare.

## One-time signing setup (required before the real run)

1. **Developer ID certificate** — needs a paid Apple Developer Program
   membership:
   - Xcode → Settings → Accounts → add Apple ID → "Manage Certificates…" →
     **+** → **Developer ID Application**.
   - Verify it is in the keychain:
     ```bash
     security find-identity -v -p codesigning
     # must list: "Developer ID Application: <Name> (<TEAMID>)"
     ```
2. **Notarization credentials** — store once as a profile named
   `rewinder-notary` (App Store Connect API key preferred):
   ```bash
   xcrun notarytool store-credentials rewinder-notary \
     --key /path/to/AuthKey_XXXXXXXX.p8 --key-id XXXXXXXX --issuer <issuer-uuid>
   ```
   Alternative (app-specific password):
   ```bash
   xcrun notarytool store-credentials rewinder-notary \
     --apple-id you@example.com --team-id TEAMID --password <app-specific-password>
   ```

## The release run

```bash
git clone git@github.com:abhinavkale-dev/rewinder.git
cd rewinder/RewinderApp
scripts/release_dmg.sh
```

The script does all of this itself (no manual steps in between):

1. Builds the Rust static library (release) and the ScreenCaptureKit helper.
2. Builds the Swift app and assembles `build/Rewinder.app`.
3. Downloads a self-contained static `ffmpeg`/`ffprobe` if not already vendored
   (`scripts/fetch_ffmpeg.sh`) and refuses to ship one that links non-system
   libraries.
4. Signs everything with the Developer ID identity (hardened runtime + secure
   timestamp), and fails if the debug `get-task-allow` entitlement is present.
5. Submits the app for notarization, staples the ticket, builds the DMG, then
   signs + notarizes + staples the DMG too.
6. Prints the Gatekeeper assessment and SHA-256, and writes both artifacts to
   `dist/`.

Expected duration: a few minutes of building plus however long Apple's
notarization queue takes (usually 1–15 minutes per submission; there are two
submissions — app and DMG).

## Known interactive gotchas (important for an unattended agent)

- **Keychain prompt during codesign:** the first signing run may pop a system
  dialog asking to allow `codesign` to use the signing key. A human must click
  **"Always Allow"** (not "Allow") or the build will hang silently at the
  `codesign` step. If the script seems stuck >5 minutes with no output, look
  for that dialog.
- **Notarization failures:** if `notarytool` rejects, run
  `xcrun notarytool log <submission-id> --keychain-profile rewinder-notary` to
  get the JSON report and fix what it lists. The most common cause is signing
  with the wrong identity (must be **Developer ID Application**, not "Apple
  Development").

## Verify before sending

```bash
cd rewinder/RewinderApp
spctl -a -t open --context context:primary-signature -v dist/Rewinder-*.dmg
# expected: "accepted • source=Notarized Developer ID"
shasum -a 256 -c dist/Rewinder-*.dmg.sha256
# expected: OK
```

Also do a smoke test: mount the DMG, drag `Rewinder.app` to `/Applications`,
launch it. First launch must show **no Gatekeeper warning**. Grant Screen
Recording + Microphone when prompted, confirm the buffer ring starts filling,
press the Save button, and check an `.mp4` lands in `~/Downloads/Rewinder`.

## Send back

```bash
cd rewinder/RewinderApp/dist
zip Rewinder-dmg.zip Rewinder-*.dmg Rewinder-*.dmg.sha256
```

Send `Rewinder-dmg.zip` back to the repo owner (they will attach the DMG to a
GitHub release and point the website's download button at it).

## Useful overrides (only if needed)

| Variable | Purpose |
| --- | --- |
| `REWINDER_CODESIGN_IDENTITY` | Pin a specific identity (auto-detected otherwise). |
| `REWINDER_NOTARY_PROFILE` | notarytool profile name (default `rewinder-notary`). |
| `REWINDER_SKIP_NOTARIZE=1` | Dry run: build + sign + DMG, no notarization. |
| `REWINDER_FFMPEG_DIR` | Use your own static ffmpeg/ffprobe build. |

Pipeline sanity check without any certificate (produces an unsigned DMG, for
testing only — never ship this):

```bash
REWINDER_SKIP_NOTARIZE=1 REWINDER_CODESIGN_IDENTITY="-" scripts/release_dmg.sh
```

## Licensing note (for the shipped DMG)

The default vendored ffmpeg build is **GPL**. If this DMG is distributed
publicly, either link the corresponding ffmpeg source (upstream `b6.1.1` tag)
alongside the download, or rebuild with an LGPL ffmpeg
(`REWINDER_FFMPEG_DIR`, `--disable-gpl`; Rewinder only needs
`h264_videotoolbox`, `aac`, and the `arnndn`/`afftdn`/`amix`/`volume`/
`aresample`/`setpts` filters).
