#!/usr/bin/env bash
# Creates the GPG key that signs the Linux release checksums.
#
# The private key is written to a file only you can read; the script never
# prints it. Put it in the LINUX_GPG_PRIVATE_KEY secret and keep a backup:
# without it, Linux packages cannot be signed.
#
# Usage: linux_keygen.sh [output-dir]
set -euo pipefail

OUT_DIR="${1:-${XDG_CONFIG_HOME:-$HOME/.config}/lumina-release}"
PRIVATE="$OUT_DIR/linux-signing-key.asc"
PUBLIC="$OUT_DIR/lumina-linux-signing-key.asc"
mkdir -p "$OUT_DIR"
chmod 700 "$OUT_DIR"
if [[ -e "$PRIVATE" ]]; then
    echo "Refusing to overwrite $PRIVATE; move it away first." >&2
    exit 1
fi
command -v gpg >/dev/null || {
    echo "gpg is not installed" >&2
    exit 1
}

# A throwaway keyring: only the exported files are kept.
GNUPGHOME="$(mktemp -d "${TMPDIR:-/tmp}/lumina-linux-key.XXXXXX")"
export GNUPGHOME
trap 'rm -rf "$GNUPGHOME"' EXIT
chmod 700 "$GNUPGHOME"
umask 077

gpg --batch --passphrase '' --quick-gen-key \
    "Lumina Linux Releases <raysca@users.noreply.github.com>" ed25519 sign 5y
FPR="$(gpg --list-keys --with-colons | awk -F: '/^fpr/{print $10; exit}')"
gpg --armor --export-secret-keys "$FPR" > "$PRIVATE"
gpg --armor --export "$FPR" > "$PUBLIC"
chmod 600 "$PRIVATE"

cat <<EOF
Private key:  $PRIVATE
Public key:   $PUBLIC
Fingerprint:  $FPR

Add the repository secrets (GitHub needs the base64 on one line):
  openssl base64 -A -in "$PRIVATE" | gh secret set LINUX_GPG_PRIVATE_KEY
  gh secret set LINUX_SIGNING_KEY --body "$FPR"

Publish the fingerprint with the release, and back up the private key.
EOF
