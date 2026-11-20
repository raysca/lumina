#!/usr/bin/env bash
# Builds, optionally notarizes, and packages Lumina.app as zip + DMG with checksums.
# Env: LUMINA_UNIVERSAL, LUMINA_SIGN_IDENTITY, LUMINA_NOTARIZE=1, notary credentials.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

./scripts/build_release.sh
if [[ "${LUMINA_NOTARIZE:-0}" == "1" ]]; then
    ./scripts/notarize_release.sh
fi

VERSION="$(grep -m1 "^version" crates/lumina-ui-gpui/Cargo.toml | sed "s/.*= \"//;s/\"//")"
OUT="target/release/bundle/osx"
APP="$OUT/Lumina.app"
ARCHS="$(lipo -archs "$APP/Contents/MacOS/Lumina")"
if [[ "$ARCHS" == *"x86_64"* && "$ARCHS" == *"arm64"* ]]; then
    SUFFIX="macos-universal"
else
    SUFFIX="macos-$ARCHS"
fi
ZIP="$OUT/Lumina-v$VERSION-$SUFFIX.zip"
DMG="$OUT/Lumina-v$VERSION.dmg"

rm -f "$ZIP" "$DMG" "$OUT/SHA256SUMS.txt"
ditto -c -k --keepParent "$APP" "$ZIP"

codesign --verify --deep --strict "$APP"

if [[ "${LUMINA_SKIP_DMG:-0}" == "1" ]]; then
    echo "LUMINA_SKIP_DMG=1: skipping DMG creation"
else
    STAGE="$OUT/dmg_stage"
    rm -rf "$STAGE"
    mkdir -p "$STAGE"
    cp -R "$APP" "$STAGE/"
    ln -s /Applications "$STAGE/Applications"
    if command -v diskutil >/dev/null 2>&1 && \
        diskutil image create from --format UDZO --volumeName "Lumina Photos" "$STAGE" "$DMG" >/dev/null 2>&1; then
        :
    elif hdiutil create -volname "Lumina Photos" -srcfolder "$STAGE" -ov -format UDZO "$DMG"; then
        :
    else
        rm -rf "$STAGE"
        echo "ERROR: could not create the DMG (hdiutil/diskutil both failed)." >&2
        echo "       This is usually a sandbox restriction; rerun on a normal session or set LUMINA_SKIP_DMG=1." >&2
        exit 1
    fi
    rm -rf "$STAGE"
    hdiutil verify "$DMG" >/dev/null
fi

FILES=("$(basename "$ZIP")")
if [[ -f "$DMG" ]]; then
    FILES+=("$(basename "$DMG")")
fi
( cd "$OUT" && shasum -a 256 "${FILES[@]}" > SHA256SUMS.txt )

echo "Artifacts:"
ls -lh "${FILES[@]/#/$OUT/}" "$OUT/SHA256SUMS.txt"
