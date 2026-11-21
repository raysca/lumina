#!/usr/bin/env bash
# Creates the project's update-signing key pair.
#
# The private key is printed nowhere; it is written to a file only you can
# read. Put it in the UPDATE_SIGNING_KEY repository secret (base64 of the PEM)
# and keep a copy in a password manager. Losing it means a build has to be
# shipped with a new public key before updates can be signed again.
#
# Usage: update_keygen.sh [private-key-path]
set -euo pipefail

OUT="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/lumina-release/update-key.pem}"
mkdir -p "$(dirname "$OUT")"
chmod 700 "$(dirname "$OUT")"
if [[ -e "$OUT" ]]; then
    echo "Refusing to overwrite $OUT; move it away first." >&2
    exit 1
fi

umask 077
openssl genpkey -algorithm ED25519 -out "$OUT"
chmod 600 "$OUT"

PUB="$(openssl pkey -in "$OUT" -pubout -outform DER | tail -c 32 | base64 | tr -d '\n')"

cat <<EOF
Private key: $OUT (mode 600)

Public key (base64):
  $PUB

Next:
  1. Put this in crates/lumina-core/src/update/manifest.rs as UPDATE_PUBLIC_KEY.
  2. Add the private key to the repository secrets:
       base64 < "$OUT"    # paste as UPDATE_SIGNING_KEY
  3. Keep a backup; do not commit the PEM.
EOF
