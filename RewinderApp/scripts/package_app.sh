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
