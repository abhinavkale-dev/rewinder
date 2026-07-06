#!/usr/bin/env bash
#
# Fetches a self-contained, statically-linked ffmpeg + ffprobe (arm64) into
# RewinderApp/vendor/ffmpeg so the packaged app does NOT depend on a user's
# Homebrew install. Rewinder only needs h264_videotoolbox (system framework),
# the built-in aac encoder, and the core avfilter filters (arnndn/afftdn/
# volume/amix/aresample/setpts) — all present in a standard static build.
#
# The default build is the prebuilt arm64 static release from
# eugeneware/ffmpeg-static (a GPL build — see DEPLOYMENT.md for the
# corresponding-source obligation). Override the URLs to point at your own
# (e.g. LGPL) build:
#
#   REWINDER_FFMPEG_URL=...  REWINDER_FFPROBE_URL=...  scripts/fetch_ffmpeg.sh
#   scripts/fetch_ffmpeg.sh --force     # re-download even if present
#
set -euo pipefail

FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"   # RewinderApp/
DEST="$APP_DIR/vendor/ffmpeg"

FFMPEG_URL="${REWINDER_FFMPEG_URL:-https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-darwin-arm64}"
FFPROBE_URL="${REWINDER_FFPROBE_URL:-https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffprobe-darwin-arm64}"

mkdir -p "$DEST"

# Skip if both binaries already verify clean (unless --force).
if [[ "$FORCE" -eq 0 && -x "$DEST/ffmpeg" && -x "$DEST/ffprobe" ]]; then
    if "$(dirname "$0")/verify_ffmpeg.sh" "$DEST" >/dev/null 2>&1; then
        echo "==> ffmpeg/ffprobe already present and portable ($DEST)"
        exit 0
    fi
fi

echo "==> Downloading static ffmpeg"
curl -fsSL --retry 3 --max-time 300 -o "$DEST/ffmpeg" "$FFMPEG_URL"
echo "==> Downloading static ffprobe"
curl -fsSL --retry 3 --max-time 300 -o "$DEST/ffprobe" "$FFPROBE_URL"
chmod +x "$DEST/ffmpeg" "$DEST/ffprobe"

echo "==> Verifying"
"$(dirname "$0")/verify_ffmpeg.sh" "$DEST"

echo "==> Done: $DEST"
