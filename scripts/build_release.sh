#!/usr/bin/env bash
# Builds Lumina.app (universal when both targets are installed), then signs it.
# Environment:
#   LUMINA_UNIVERSAL=0        build only the host architecture
#   LUMINA_SIGN_IDENTITY=...  Developer ID identity; default "-" (ad-hoc)
#   LUMINA_BUNDLE_DEPS=0      skip bundling yt-dlp and ffmpeg (local testing only)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
# shellcheck source=scripts/lib/common.sh
. "$ROOT_DIR/scripts/lib/common.sh"

BIN_NAME="Lumina"
BUNDLE_DIR="target/release/bundle/osx/Lumina.app"
APP_BIN="$BUNDLE_DIR/Contents/MacOS/$BIN_NAME"
ARM_TARGET="aarch64-apple-darwin"
INTEL_TARGET="x86_64-apple-darwin"
UNIVERSAL="${LUMINA_UNIVERSAL:-1}"
SIGN_IDENTITY="${LUMINA_SIGN_IDENTITY:--}"

VERSION="$(crate_version crates/lumina-ui-gpui/Cargo.toml)"
PLIST_VERSION="$(plist_version resources/Info.plist)"
if [[ "$VERSION" != "$PLIST_VERSION" ]]; then
    echo "Version mismatch: Cargo.toml=$VERSION Info.plist=$PLIST_VERSION" >&2
    exit 1
fi
echo "Building Lumina $VERSION"

INSTALLED="$(rustup target list --installed)"
HOST="$(rustc -vV | sed -n "s/^host: //p")"

if [[ "$UNIVERSAL" == "1" && "$INSTALLED" == *"$ARM_TARGET"* && "$INSTALLED" == *"$INTEL_TARGET"* ]]; then
    echo "Building universal (arm64 + x86_64)..."
    cargo build --release -p lumina-ui-gpui --bin "$BIN_NAME" --target "$ARM_TARGET"
    cargo build --release -p lumina-ui-gpui --bin "$BIN_NAME" --target "$INTEL_TARGET"
    mkdir -p target/release/universal
    lipo -create -output "target/release/universal/$BIN_NAME" \
        "target/$ARM_TARGET/release/$BIN_NAME" \
        "target/$INTEL_TARGET/release/$BIN_NAME"
    lipo -info "target/release/universal/$BIN_NAME"
    BUILT_BIN="target/release/universal/$BIN_NAME"
else
    if [[ "$UNIVERSAL" == "1" ]]; then
        echo "WARNING: universal targets not installed; building only $HOST" >&2
        echo "         run: rustup target add $ARM_TARGET $INTEL_TARGET" >&2
    fi
    cargo build --release -p lumina-ui-gpui --bin "$BIN_NAME" --target "$HOST"
    BUILT_BIN="target/$HOST/release/$BIN_NAME"
fi

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS" "$BUNDLE_DIR/Contents/Resources"
cp "$BUILT_BIN" "$APP_BIN"
chmod +x "$APP_BIN"
cp resources/Info.plist "$BUNDLE_DIR/Contents/Info.plist"
cp resources/Lumina.icns "$BUNDLE_DIR/Contents/Resources/Lumina.icns"
# About → MIT License / Acknowledgements open these.
install -m 644 LICENSE THIRD_PARTY_NOTICES.md "$BUNDLE_DIR/Contents/Resources/"

# Bundle the pinned yt-dlp and ffmpeg (network import). The fetch scripts
# verify their SHA-256 and fail the build on any error; LUMINA_BUNDLE_DEPS=0
# builds without them for local UI work only. A `.app` never falls back to a
# Homebrew or PATH tool, so URL import in such a build reports a missing
# helper until it is rebuilt with the deps.
if [[ "${LUMINA_BUNDLE_DEPS:-1}" == "1" ]]; then
    ./scripts/fetch_yt_dlp.sh
    echo "Bundling yt-dlp into $BUNDLE_DIR/Contents/MacOS/yt-dlp"
    install -m 755 target/deps/yt-dlp "$BUNDLE_DIR/Contents/MacOS/yt-dlp"
    # The licenses of the runtime and packages its build bundles travel with it.
    install -m 644 target/deps/yt-dlp-THIRD_PARTY_LICENSES.txt "$BUNDLE_DIR/Contents/Resources/"
    if [[ "$SIGN_IDENTITY" != "-" ]]; then
        codesign --force --options runtime --timestamp \
            --entitlements resources/yt-dlp.entitlements \
            --sign "$SIGN_IDENTITY" "$BUNDLE_DIR/Contents/MacOS/yt-dlp"
    else
        codesign --force --sign - "$BUNDLE_DIR/Contents/MacOS/yt-dlp"
    fi
    # A signing change that stops the helper from starting must fail the build.
    if ! "$BUNDLE_DIR/Contents/MacOS/yt-dlp" --ignore-config --version >/dev/null; then
        echo "error: the signed yt-dlp does not run" >&2
        exit 1
    fi

    ./scripts/fetch_ffmpeg.sh
    echo "Bundling ffmpeg into $BUNDLE_DIR/Contents/MacOS/ffmpeg"
    install -m 755 target/deps/ffmpeg "$BUNDLE_DIR/Contents/MacOS/ffmpeg"
    # LGPL-2.1: the license, and the exact source and build script, travel
    # with the binary.
    install -m 644 target/deps/ffmpeg-LICENSE.txt "$BUNDLE_DIR/Contents/Resources/"
    FFMPEG_VERSION="$(sed -n 's/^FFMPEG_VERSION="\(.*\)"$/\1/p' scripts/fetch_ffmpeg.sh)"
    mkdir -p "$BUNDLE_DIR/Contents/Resources/ffmpeg-source"
    install -m 644 "target/deps/src/ffmpeg-$FFMPEG_VERSION.tar.xz" scripts/fetch_ffmpeg.sh \
        "$BUNDLE_DIR/Contents/Resources/ffmpeg-source/"
    if [[ "$SIGN_IDENTITY" != "-" ]]; then
        codesign --force --options runtime --timestamp \
            --sign "$SIGN_IDENTITY" "$BUNDLE_DIR/Contents/MacOS/ffmpeg"
    else
        codesign --force --sign - "$BUNDLE_DIR/Contents/MacOS/ffmpeg"
    fi
    if ! "$BUNDLE_DIR/Contents/MacOS/ffmpeg" -hide_banner -version >/dev/null; then
        echo "error: the signed ffmpeg does not run" >&2
        exit 1
    fi
else
    echo "warning: LUMINA_BUNDLE_DEPS=0: yt-dlp and ffmpeg not bundled; not a distributable release" >&2
fi

if [[ "$SIGN_IDENTITY" != "-" ]]; then
    echo "Signing with identity: $SIGN_IDENTITY"
    codesign --force --options runtime --timestamp \
        --entitlements resources/Lumina.entitlements \
        --sign "$SIGN_IDENTITY" "$APP_BIN"
    codesign --force --options runtime --timestamp \
        --entitlements resources/Lumina.entitlements \
        --sign "$SIGN_IDENTITY" "$BUNDLE_DIR"
else
    echo "No signing identity supplied; applying ad-hoc signature"
    codesign --force --sign - "$APP_BIN"
    codesign --force --sign - "$BUNDLE_DIR"
fi

codesign --verify --deep --strict --verbose=2 "$BUNDLE_DIR"
echo "Bundle ready at $BUNDLE_DIR"
