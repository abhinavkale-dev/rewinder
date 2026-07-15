#!/usr/bin/env bash
#
# Assembles Rewinder.app from the Swift executable + Rust static library and the
# helper/ffmpeg binaries. Signs with a stable Apple Development identity when one
# is available (so macOS permission grants survive rebuilds), falling back to
# ad-hoc otherwise. Developer-ID signing and notarization for distribution are
# deferred (require Apple credentials) — see README.md.
#
# Usage:
#   scripts/package_app.sh            # release build (default)
#   scripts/package_app.sh --debug    # faster debug build for iteration
#
set -euo pipefail

MODE="release"
if [[ "${1:-}" == "--debug" ]]; then
    MODE="debug"
fi

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"   # RewinderApp/
REPO_DIR="$(cd "$APP_DIR/.." && pwd)"         # repo root
OUT="$APP_DIR/build/Rewinder.app"
CONTENTS="$OUT/Contents"

echo "==> Building Rust static library ($MODE)"
if [[ "$MODE" == "release" ]]; then
    ( cd "$REPO_DIR/src-tauri" && cargo build --release --lib )
    SWIFT_FLAGS=(-c release)
    SWIFT_BIN="$APP_DIR/.build/release/RewinderApp"
else
    ( cd "$REPO_DIR/src-tauri" && cargo build --lib )
    SWIFT_FLAGS=()
    SWIFT_BIN="$APP_DIR/.build/debug/RewinderApp"
fi

echo "==> Building Swift app ($MODE)"
rm -f "$SWIFT_BIN"
( cd "$APP_DIR" && REWINDER_RUST_PROFILE="$MODE" swift build ${SWIFT_FLAGS[@]+"${SWIFT_FLAGS[@]}"} )

echo "==> Laying out app bundle"
rm -rf "$OUT"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources/bin"
cp "$SWIFT_BIN" "$CONTENTS/MacOS/Rewinder"
cp "$APP_DIR/Resources/Info.plist" "$CONTENTS/Info.plist"
cp "$APP_DIR/Resources/AppIcon.icns" "$CONTENTS/Resources/AppIcon.icns"
# Also ship the PNG: notification attachments load the logo by resource name.
cp "$APP_DIR/Resources/AppIcon.png" "$CONTENTS/Resources/AppIcon.png"
cp "$APP_DIR/Resources/TrayIcon.png" "$CONTENTS/Resources/TrayIcon.png"

# UI cues (save chime / denied blip) — rendered from the same synth recipes as
# the website hero so the app and site sound identical.
if compgen -G "$APP_DIR/Resources/Sounds/*.wav" >/dev/null; then
    mkdir -p "$CONTENTS/Resources/Sounds"
    cp "$APP_DIR/Resources/Sounds/"*.wav "$CONTENTS/Resources/Sounds/"
fi

echo "==> Compiling Liquid Glass app icon (Icon Composer .icon)"
# macOS 26 renders the dynamic glass icon from the asset catalog that actool
# produces (keyed by CFBundleIconName=Rewinder). AppIcon.icns stays as the
# fallback for any surface that can't read the .icon.
ICON_SRC="$APP_DIR/Resources/Rewinder.icon"
if [[ -d "$ICON_SRC" ]] && xcrun --find actool >/dev/null 2>&1; then
    ICON_BUILD="$(mktemp -d)"
    if xcrun actool "$ICON_SRC" \
        --compile "$ICON_BUILD" \
        --app-icon Rewinder \
        --output-partial-info-plist "$ICON_BUILD/partial.plist" \
        --platform macosx \
        --minimum-deployment-target 26.0 \
        --errors --warnings >/dev/null 2>&1 && [[ -f "$ICON_BUILD/Assets.car" ]]; then
        cp "$ICON_BUILD/Assets.car" "$CONTENTS/Resources/Assets.car"
        [[ -f "$ICON_BUILD/Rewinder.icns" ]] && \
            cp "$ICON_BUILD/Rewinder.icns" "$CONTENTS/Resources/Rewinder.icns"
        echo "    bundled Assets.car (dynamic glass icon)"
    else
        echo "WARN: actool could not compile Rewinder.icon; using AppIcon.icns only"
    fi
    rm -rf "$ICON_BUILD"
else
    echo "WARN: actool/Rewinder.icon unavailable; using AppIcon.icns only"
fi

echo "==> Bundling sck_capture helper"
HELPER="$REPO_DIR/src-tauri/target/sck-helper/rewinder-sck-capture"
if [[ -x "$HELPER" ]]; then
    cp "$HELPER" "$CONTENTS/Resources/bin/rewinder-sck-capture"
else
    echo "WARN: helper not found at $HELPER (build the Rust lib first)"
fi

echo "==> Bundling RNNoise model"
RNNOISE_MODEL="$REPO_DIR/src-tauri/resources/bd.rnnn"
if [[ -f "$RNNOISE_MODEL" ]]; then
    mkdir -p "$CONTENTS/Resources/models"
    cp "$RNNOISE_MODEL" "$CONTENTS/Resources/models/bd.rnnn"
else
    echo "WARN: RNNoise model not found at $RNNOISE_MODEL; mic noise"
    echo "      suppression will fall back to the basic afftdn denoiser."
fi

echo "==> Bundling ffmpeg/ffprobe (self-contained static)"
# Ship a static, fully self-contained ffmpeg/ffprobe so the app works on any
# user's Mac (a Homebrew ffmpeg links /opt/homebrew dylibs that won't exist).
# Defaults to the vendored arm64 static build; auto-fetches it if missing.
# Override REWINDER_FFMPEG_DIR to point at your own (e.g. LGPL) static build.
FFMPEG_DIR="${REWINDER_FFMPEG_DIR:-$APP_DIR/vendor/ffmpeg}"
if [[ ! -x "$FFMPEG_DIR/ffmpeg" || ! -x "$FFMPEG_DIR/ffprobe" ]]; then
    if [[ "$FFMPEG_DIR" == "$APP_DIR/vendor/ffmpeg" ]]; then
        echo "    vendored ffmpeg missing; fetching a static build"
        "$APP_DIR/scripts/fetch_ffmpeg.sh"
    else
        echo "ERROR: ffmpeg/ffprobe not found in REWINDER_FFMPEG_DIR=$FFMPEG_DIR" >&2
        exit 1
    fi
fi
cp "$FFMPEG_DIR/ffmpeg" "$CONTENTS/Resources/bin/ffmpeg"
cp "$FFMPEG_DIR/ffprobe" "$CONTENTS/Resources/bin/ffprobe"

# Portability guard: refuse to ship an ffmpeg that links non-system libraries
# (the #1 way a packaged app breaks on machines without Homebrew).
"$APP_DIR/scripts/verify_ffmpeg.sh" "$CONTENTS/Resources/bin"

echo "==> Code signing"
# macOS ties permission grants (Screen Recording, Microphone) to the app's code
# signing identity. Ad-hoc ('-') mints a fresh identity every build, so each
# rebuild orphans previously granted permissions. Prefer a stable Apple
# Development identity so grants survive rebuilds. Override with
# REWINDER_CODESIGN_IDENTITY=...; falls back to ad-hoc if none is available.
SIGN_ID="${REWINDER_CODESIGN_IDENTITY:-}"
if [[ -z "$SIGN_ID" ]]; then
    SIGN_ID="$(security find-identity -v -p codesigning 2>/dev/null \
        | awk -F'"' '/Apple Development|Developer ID Application/ {print $2; exit}')"
fi
# Local fallback: a self-signed "Rewinder Dev" cert (create one in Keychain
# Access as a code-signing certificate). Not distributable, but stable — so TCC
# permission grants survive rebuilds even without an Apple identity.
if [[ -z "$SIGN_ID" ]]; then
    SIGN_ID="$(security find-identity -v -p codesigning 2>/dev/null \
        | awk -F'"' '/Rewinder Dev/ {print $2; exit}')"
fi
if [[ -z "$SIGN_ID" ]]; then
    SIGN_ID="-"
    echo "    WARN: no Apple Development identity found; using ad-hoc signing."
    echo "          Permissions will reset on every rebuild. Set"
    echo "          REWINDER_CODESIGN_IDENTITY to a stable identity to avoid this."
else
    echo "    identity: $SIGN_ID"
fi

# A real identity gets the hardened runtime + a secure timestamp (both required
# for notarization). Ad-hoc ('-') and the self-signed "Rewinder Dev" cert get
# neither (timestamps need an Apple-issued identity; the runtime offers nothing
# for local dev builds).
RUNTIME_OPT=(--options runtime)
TIMESTAMP_OPT=(--timestamp)
if [[ "$SIGN_ID" == "-" || "$SIGN_ID" == "Rewinder Dev" ]]; then
    RUNTIME_OPT=()
    TIMESTAMP_OPT=()
fi
SIGN_OPTS=()
SIGN_OPTS+=(${RUNTIME_OPT[@]+"${RUNTIME_OPT[@]}"})
SIGN_OPTS+=(${TIMESTAMP_OPT[@]+"${TIMESTAMP_OPT[@]}"})

# ffmpeg/ffprobe are now self-contained static binaries (verified above), so they
# can carry the hardened runtime like the rest of the bundle — required for
# notarization. (The old Homebrew build had to skip the runtime because it loaded
# third-party dylibs; that no longer applies.)
for tool in ffmpeg ffprobe; do
    BIN="$CONTENTS/Resources/bin/$tool"
    [[ -f "$BIN" ]] && codesign --force ${SIGN_OPTS[@]+"${SIGN_OPTS[@]}"} --sign "$SIGN_ID" "$BIN"
done

# The sck_capture helper is our own code: sign it like the app.
HELPER_BIN="$CONTENTS/Resources/bin/rewinder-sck-capture"
[[ -f "$HELPER_BIN" ]] && \
    codesign --force ${SIGN_OPTS[@]+"${SIGN_OPTS[@]}"} --sign "$SIGN_ID" "$HELPER_BIN"

# Any other nested binaries (none expected today): sign defensively.
for bin in "$CONTENTS/Resources/bin/"*; do
    case "$(basename "$bin")" in
        ffmpeg|ffprobe|rewinder-sck-capture) continue ;;
    esac
    [[ -f "$bin" ]] && codesign --force ${SIGN_OPTS[@]+"${SIGN_OPTS[@]}"} --sign "$SIGN_ID" "$bin" || true
done

# Outer app: hardened runtime + secure timestamp + entitlements, signed last.
codesign --force ${SIGN_OPTS[@]+"${SIGN_OPTS[@]}"} --sign "$SIGN_ID" \
    --entitlements "$APP_DIR/Resources/Rewinder.entitlements" \
    "$OUT"

echo "==> Done: $OUT"
