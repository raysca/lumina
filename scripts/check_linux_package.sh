#!/usr/bin/env bash
# Verifies the built Debian package: checksum, glibc floor, install, launch
# under X11 (scripts/linux_package_smoke.sh), and removal without leftovers
# while the user's files and config stay. Needs root (sudo) and xvfb-run.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
OUT_DIR="target/release/bundle/linux"

(cd "$OUT_DIR" && sha256sum --check SHA256SUMS.txt)

# The package the checksum file names, not any stale .deb lying around.
DEB="$OUT_DIR/$(awk '/\.deb$/ {print $2; exit}' "$OUT_DIR/SHA256SUMS.txt")"
[[ -f "$DEB" ]] || { echo "No checksummed .deb in $OUT_DIR" >&2; exit 1; }

LIBC_DEP="$(dpkg-deb -f "$DEB" Depends | grep -oE 'libc6 \(>= [0-9.]+\)' | head -1)"
if [[ "$LIBC_DEP" != "libc6 (>= 2.39)" ]]; then
    echo "Unexpected glibc dependency: '${LIBC_DEP:-none}'" >&2
    exit 1
fi

sudo apt-get install -y "$ROOT_DIR/$DEB"
command -v lumina >/dev/null || { echo "lumina is not on PATH after install" >&2; exit 1; }

SMOKE_HOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-package-check.XXXXXX")"
trap 'rm -rf "$SMOKE_HOME"' EXIT
mkdir -p "$SMOKE_HOME/user-library"
# A file of the user's own, apart from anything the smoke writes.
printf 'keep this\n' > "$SMOKE_HOME/user-library/keep-after-remove.txt"
LUMINA_PACKAGE_SMOKE_HOME="$SMOKE_HOME" xvfb-run -a ./scripts/linux_package_smoke.sh

sudo apt-get remove -y lumina
if command -v lumina >/dev/null; then
    echo "lumina is still on PATH after removal" >&2
    exit 1
fi
for leftover in /usr/share/applications/lumina.desktop \
    /usr/share/icons/hicolor/512x512/apps/lumina.png; do
    if [[ -e "$leftover" ]]; then
        echo "Package removal left $leftover" >&2
        exit 1
    fi
done
test -f "$SMOKE_HOME/user-library/keep-after-remove.txt" \
    || { echo "Package removal touched the user's files" >&2; exit 1; }
test -f "$SMOKE_HOME/config/lumina/config.json" \
    || { echo "The user's config is missing after removal" >&2; exit 1; }
echo "PASS: package installs, launches and removes cleanly"
