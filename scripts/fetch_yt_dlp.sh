#!/usr/bin/env bash
# Fetches the pinned yt-dlp release into target/deps/yt-dlp, and the licenses
# of what its build bundles into target/deps/yt-dlp-THIRD_PARTY_LICENSES.txt,
# and verifies both SHA-256s. Bump all three values together: the binary's
# hash from the release's SHA2-256SUMS file, the licenses file's from
# `shasum -a 256` on THIRD_PARTY_LICENSES.txt at the release tag.
set -euo pipefail

YTDLP_VERSION="2026.10.26"
YTDLP_SHA256="0f922c15ccc0b83eefa1c8bc9104e434318c4bf16e408e1ee93b7745a6efe0aa"
LICENSES_SHA256="472aefe951c7db35e1657c1d13fd337140511ed6f2b329205105ad441c5a02b7"
DEST="target/deps/yt-dlp"
LICENSES="target/deps/yt-dlp-THIRD_PARTY_LICENSES.txt"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

matches() { [[ -f "$1" ]] && echo "$2  $1" | shasum -a 256 -c --status; }

# fetch FILE SHA256 URL
fetch() {
    if matches "$1" "$2"; then
        return
    fi
    mkdir -p "$(dirname "$1")"
    echo "Fetching $(basename "$1") for yt-dlp $YTDLP_VERSION..."
    curl -f -L -o "$1.download" "$3"
    mv "$1.download" "$1"
    if ! matches "$1" "$2"; then
        rm -f "$1"
        echo "error: $(basename "$1") for yt-dlp $YTDLP_VERSION does not match the pinned SHA-256" >&2
        exit 1
    fi
}

fetch "$DEST" "$YTDLP_SHA256" \
    "https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VERSION/yt-dlp_macos"
chmod +x "$DEST"
# Not a release asset: the file lives in the source tree at the release tag.
fetch "$LICENSES" "$LICENSES_SHA256" \
    "https://raw.githubusercontent.com/yt-dlp/yt-dlp/$YTDLP_VERSION/THIRD_PARTY_LICENSES.txt"
