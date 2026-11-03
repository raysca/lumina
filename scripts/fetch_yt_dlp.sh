#!/usr/bin/env bash
# Fetches the pinned yt-dlp release into target/deps/yt-dlp and verifies its
# SHA-256. Bump YTDLP_VERSION and YTDLP_SHA256 together, taking the hash from
# the release's SHA2-256SUMS file.
set -euo pipefail

YTDLP_VERSION="2026.10.26"
YTDLP_SHA256="0f922c15ccc0b83eefa1c8bc9104e434318c4bf16e408e1ee93b7745a6efe0aa"
DEST="target/deps/yt-dlp"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

matches() { [[ -f "$DEST" ]] && echo "$YTDLP_SHA256  $DEST" | shasum -a 256 -c --status; }

if ! matches; then
    mkdir -p "$(dirname "$DEST")"
    echo "Fetching yt-dlp $YTDLP_VERSION..."
    curl -f -L -o "$DEST.download" \
        "https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VERSION/yt-dlp_macos"
    mv "$DEST.download" "$DEST"
    if ! matches; then
        rm -f "$DEST"
        echo "error: yt-dlp $YTDLP_VERSION does not match the pinned SHA-256" >&2
        exit 1
    fi
fi
chmod +x "$DEST"
