#!/usr/bin/env bash
# Signs the Linux release checksums with the release key and verifies the
# signature, and exports the matching public key beside them.
#
# Usage: sign_linux_release.sh <release-dir>
#
# Environment:
#   LINUX_SIGNING_KEY          fingerprint (or key id) of the signing key
#   LINUX_SIGNING_PASSPHRASE   optional; needed for a protected key
#   GNUPGHOME                  gpg home holding the secret key (CI sets it)
#
# The package itself is not signed in-band: `debsigs` 0.01.19 (Ubuntu 24.04)
# implements signing but not verification, so a detached signature over the
# checksums is what users can actually check. A signed apt repository is the
# next step for `apt`-based delivery.
set -euo pipefail

DIR="${1:?usage: sign_linux_release.sh <release-dir>}"
SUMS="$DIR/SHA256SUMS-linux.txt"
KEY="${LINUX_SIGNING_KEY:-}"

[[ -f "$SUMS" ]] || {
    echo "Missing $SUMS; generate the checksums before signing" >&2
    exit 1
}
[[ -n "$KEY" ]] || {
    echo "LINUX_SIGNING_KEY is not set; refusing to sign with an arbitrary key" >&2
    exit 1
}
command -v gpg >/dev/null || {
    echo "gpg is not installed" >&2
    exit 1
}

gpg_args=(--batch --yes --pinentry-mode loopback --local-user "$KEY")
if [[ -n "${LINUX_SIGNING_PASSPHRASE:-}" ]]; then
    gpg_args+=(--passphrase "$LINUX_SIGNING_PASSPHRASE")
fi

gpg "${gpg_args[@]}" --armor --detach-sign \
    --output "$SUMS.asc" "$SUMS"
gpg --batch --verify "$SUMS.asc" "$SUMS"

# Publish the public half next to the artifacts so a download can be checked
# without fetching the key from a keyserver.
gpg --batch --armor --export "$KEY" > "$DIR/lumina-linux-signing-key.asc"
[[ -s "$DIR/lumina-linux-signing-key.asc" ]] || {
    echo "Could not export the public key for $KEY" >&2
    exit 1
}

echo "Signed $SUMS"
echo "Public key: $DIR/lumina-linux-signing-key.asc"
