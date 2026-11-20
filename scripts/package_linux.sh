#!/usr/bin/env bash
# Build a native Debian package for supported Ubuntu/Debian Linux systems.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ARCH="$(dpkg --print-architecture)"
case "$ARCH" in
    amd64|arm64) ;;
    *) echo "Unsupported Debian architecture: $ARCH" >&2; exit 1 ;;
esac

crate_version() {
    sed -n 's/^version = "\([^"]*\)"/\1/p' "$1" | head -1
}
VERSION="$(crate_version crates/lumina-ui-gpui/Cargo.toml)"
CORE_VERSION="$(crate_version crates/lumina-core/Cargo.toml)"
MAKE_VERSION="$(sed -n 's/^VERSION := //p' Makefile | head -1)"
# No PlistBuddy on Linux: the value is the <string> after the key.
PLIST_VERSION="$(grep -A1 '<key>CFBundleShortVersionString</key>' resources/Info.plist \
    | sed -n 's/.*<string>\(.*\)<\/string>.*/\1/p')"
if [[ -z "$VERSION" || "$VERSION" != "$CORE_VERSION" || "$VERSION" != "$MAKE_VERSION" \
    || "$VERSION" != "$PLIST_VERSION" ]]; then
    echo "Version mismatch: lumina-ui-gpui=$VERSION lumina-core=$CORE_VERSION Makefile=$MAKE_VERSION Info.plist=$PLIST_VERSION" >&2
    exit 1
fi
OUT_DIR="target/release/bundle/linux"
PKG_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/lumina-deb.XXXXXX")"
trap 'rm -rf "$PKG_ROOT"' EXIT

APP_ID="io.github.raysca.Lumina"
DESKTOP="resources/linux/${APP_ID}.desktop"
METAINFO="resources/linux/${APP_ID}.metainfo.xml"

# The desktop entry and AppStream metadata are part of the package contract:
# validate them here so a broken one never reaches a build artifact.
for tool in desktop-file-validate appstreamcli; do
    command -v "$tool" >/dev/null || {
        echo "Missing $tool; install desktop-file-utils and appstream" >&2
        exit 1
    }
done
desktop-file-validate "$DESKTOP"
appstreamcli validate --no-net "$METAINFO"

cargo build --release -p lumina-ui-gpui --bin Lumina

install -Dm755 target/release/Lumina "$PKG_ROOT/usr/bin/lumina"
install -Dm644 resources/app_icon.png \
    "$PKG_ROOT/usr/share/icons/hicolor/512x512/apps/${APP_ID}.png"
install -Dm644 "$DESKTOP" "$PKG_ROOT/usr/share/applications/${APP_ID}.desktop"
install -Dm644 "$METAINFO" "$PKG_ROOT/usr/share/metainfo/${APP_ID}.metainfo.xml"
install -Dm644 LICENSE "$PKG_ROOT/usr/share/doc/lumina/copyright"
# The bundled fonts and icon set ship beside the app; their licenses must
# travel with them.
for license in resources/fonts/Inter-LICENSE.txt \
    resources/fonts/InterTight-OFL.txt resources/fonts/JetBrainsMono-OFL.txt \
    resources/icons/LICENSE; do
    install -Dm644 "$license" \
        "$PKG_ROOT/usr/share/doc/lumina/licenses/$(basename "$license")"
done

install -d "$PKG_ROOT/DEBIAN" "$OUT_DIR"
mkdir -p "$PKG_ROOT/debian"
cat > "$PKG_ROOT/debian/control" <<EOF
Source: lumina
Package: lumina
Architecture: $ARCH
Description: Local-first photo and video manager
 Lumina catalogs and manages local photos and videos.
EOF
SHLIB_DEPENDS="$(cd "$PKG_ROOT" && dpkg-shlibdeps -O -e"$PKG_ROOT/usr/bin/lumina" \
    | sed -n 's/^shlibs:Depends=//p')"
if [[ -z "$SHLIB_DEPENDS" ]]; then
    echo "Could not determine the binary's shared-library dependencies" >&2
    exit 1
fi
rm -rf "$PKG_ROOT/debian"

cat > "$PKG_ROOT/DEBIAN/control" <<EOF
Package: lumina
Version: $VERSION
Section: graphics
Priority: optional
Architecture: $ARCH
Depends: $SHLIB_DEPENDS, gstreamer1.0-plugins-base, gstreamer1.0-plugins-good, gstreamer1.0-libav, ffmpeg, curl
Recommends: yt-dlp
Maintainer: Lumina maintainers
Description: Local-first photo and video manager
 Lumina catalogs and manages local photos and videos.
EOF

dpkg-deb --root-owner-group --build "$PKG_ROOT" "$OUT_DIR/lumina_${VERSION}_${ARCH}.deb"
(cd "$OUT_DIR" && sha256sum "lumina_${VERSION}_${ARCH}.deb" > SHA256SUMS.txt)
echo "Package: $OUT_DIR/lumina_${VERSION}_${ARCH}.deb"
