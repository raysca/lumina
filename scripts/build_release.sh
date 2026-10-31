#!/usr/bin/env bash
# Builds Lumina.app (universal when both targets are installed), then signs it.
# Environment:
#   LUMINA_UNIVERSAL=0        build only the host architecture
#   LUMINA_SIGN_IDENTITY=...  Developer ID identity; default "-" (ad-hoc)
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

BIN_NAME="lumina-ui-gpui"
BUNDLE_DIR="target/release/bundle/osx/Lumina.app"
APP_BIN="$BUNDLE_DIR/Contents/MacOS/$BIN_NAME"
ARM_TARGET="aarch64-apple-darwin"
INTEL_TARGET="x86_64-apple-darwin"
UNIVERSAL="${LUMINA_UNIVERSAL:-1}"
SIGN_IDENTITY="${LUMINA_SIGN_IDENTITY:--}"

VERSION="$(grep -m1 "^version" crates/lumina-ui-gpui/Cargo.toml | sed "s/.*= \"//;s/\"//")"
PLIST_VERSION="$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" resources/Info.plist)"
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
