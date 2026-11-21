#!/usr/bin/env bash
# Signs an update manifest with the release key and writes <manifest>.sig,
# base64 of the detached Ed25519 signature (what the app verifies).
#
# Usage: sign_update_manifest.sh <update.json> [private-key.pem]
# Env:   UPDATE_SIGNING_KEY  base64 of the PEM, for CI (no file needed)
set -euo pipefail

MANIFEST="${1:?usage: sign_update_manifest.sh <update.json> [private-key.pem]}"
[[ -f "$MANIFEST" ]] || {
    echo "No manifest at $MANIFEST" >&2
    exit 1
}
command -v openssl >/dev/null || {
    echo "openssl is not installed" >&2
    exit 1
}

CLEANUP=()
if [[ -n "${UPDATE_SIGNING_KEY:-}" ]]; then
    KEY="$(mktemp "${TMPDIR:-/tmp}/lumina-update-key.XXXXXX")"
    CLEANUP+=("$KEY")
    printf '%s' "$UPDATE_SIGNING_KEY" | openssl base64 -d -A > "$KEY"
    chmod 600 "$KEY"
else
    KEY="${2:?set UPDATE_SIGNING_KEY or pass the private key path}"
    [[ -f "$KEY" ]] || {
        echo "No private key at $KEY" >&2
        exit 1
    }
fi
trap 'rm -f "${CLEANUP[@]}"' EXIT

SIG_BIN="$(mktemp "${TMPDIR:-/tmp}/lumina-update-sig.XXXXXX")"
CLEANUP+=("$SIG_BIN")
openssl pkeyutl -sign -inkey "$KEY" -rawin -in "$MANIFEST" -out "$SIG_BIN"
openssl base64 -A -in "$SIG_BIN" > "$MANIFEST.sig"

echo "Signed $MANIFEST -> $MANIFEST.sig"
