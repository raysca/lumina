#!/usr/bin/env bash
# Builds the ffmpeg bundled for network import into target/deps/ffmpeg: a
# universal (arm64 + x86_64), LGPL-2.1-only build from the pinned source
# tarball, configured with just what yt-dlp needs to merge video and audio and
# to turn HLS (MPEG-TS) into MP4 without re-encoding. No GPL or nonfree parts,
# no network protocols, no encoders.
#
# Bump FFMPEG_VERSION and FFMPEG_SHA256 together; cross-check the hash (e.g.
# against Homebrew's ffmpeg formula) before committing it.
set -euo pipefail

FFMPEG_VERSION="9.0.2"
FFMPEG_SHA256="8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e"
MIN_MACOS="12.0"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DEPS="target/deps"
TARBALL="$DEPS/src/ffmpeg-$FFMPEG_VERSION.tar.xz"
DEST="$DEPS/ffmpeg"
STAMP="$DEPS/ffmpeg.stamp"

CONFIGURE_FLAGS=(
    --disable-everything --disable-autodetect --disable-network
    --disable-doc --disable-debug --disable-ffplay --disable-ffprobe
    --disable-x86asm --enable-zlib
    --enable-protocol=file,pipe
    --enable-demuxer=mov,matroska,mpegts,aac,mp3,h264,hevc
    --enable-muxer=mp4,mov,ipod
    --enable-parser=h264,hevc,aac,av1,vp9,opus,mpegaudio,ac3
    # vp9: without it the MP4's VP9 config box is left empty (an unplayable
    # file, exit 0).
    --enable-decoder=aac,h264,hevc,vp9
    --enable-bsf=aac_adtstoasc,extract_extradata,vp9_superframe,null
)

# Rebuild only when the version or configuration changes.
WANT="$FFMPEG_VERSION $FFMPEG_SHA256 ${CONFIGURE_FLAGS[*]}"
if [[ -x "$DEST" && -f "$STAMP" && "$(cat "$STAMP")" == "$WANT" ]]; then
    exit 0
fi

verified() { [[ -f "$TARBALL" ]] && echo "$FFMPEG_SHA256  $TARBALL" | shasum -a 256 -c --status; }
if ! verified; then
    mkdir -p "$(dirname "$TARBALL")"
    echo "Fetching ffmpeg $FFMPEG_VERSION source..."
    curl -f -L -o "$TARBALL.download" "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
    mv "$TARBALL.download" "$TARBALL"
    if ! verified; then
        rm -f "$TARBALL"
        echo "error: ffmpeg $FFMPEG_VERSION source does not match the pinned SHA-256" >&2
        exit 1
    fi
fi

BUILD="$DEPS/build"
rm -rf "$BUILD/ffmpeg-$FFMPEG_VERSION"
mkdir -p "$BUILD"
tar -xJf "$TARBALL" -C "$BUILD"
SRC="$ROOT_DIR/$BUILD/ffmpeg-$FFMPEG_VERSION"

JOBS="$(sysctl -n hw.ncpu)"
SLICES=()
for ARCH in arm64 x86_64; do
    OUT="$ROOT_DIR/$BUILD/ffmpeg-$ARCH"
    rm -rf "$OUT"
    mkdir -p "$OUT"
    echo "Building ffmpeg $FFMPEG_VERSION for $ARCH..."
    (
        cd "$OUT"
        "$SRC/configure" "${CONFIGURE_FLAGS[@]}" \
            --arch="$ARCH" --target-os=darwin --enable-cross-compile \
            --cc="clang -arch $ARCH" \
            --extra-cflags="-mmacosx-version-min=$MIN_MACOS" \
            --extra-ldflags="-mmacosx-version-min=$MIN_MACOS" >/dev/null
        make -j"$JOBS" ffmpeg >/dev/null
    )
    SLICES+=("$OUT/ffmpeg")
done

lipo -create "${SLICES[@]}" -output "$DEST"
chmod +x "$DEST"
# LGPL: ship the license with the binary (see THIRD_PARTY_NOTICES.md).
cp "$SRC/COPYING.LGPLv2.1" "$DEPS/ffmpeg-LICENSE.txt"
"$DEST" -hide_banner -version >/dev/null
echo "$WANT" > "$STAMP"
echo "Built $DEST ($(lipo -archs "$DEST"))"
