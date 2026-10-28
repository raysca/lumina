#!/usr/bin/env bash
# Submits Lumina.app for notarization and staples the ticket.
# Requires a Developer ID signature and either:
#   LUMINA_NOTARY_PROFILE (preferred, from notarytool store-credentials)
# or APPLE_ID + APPLE_TEAM_ID + APPLE_APP_SPECIFIC_PASSWORD.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP="target/release/bundle/osx/Lumina.app"
if [[ ! -d "$APP" ]]; then
    echo "Missing $APP; run ./scripts/build_release.sh first" >&2
    exit 1
fi
if ! codesign -dv "$APP" 2>&1 | grep -q "Developer ID Application"; then
    echo "App is not Developer ID signed; notarization will be rejected." >&2
    exit 1
fi

ZIP="target/release/bundle/osx/Lumina-notarize.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

if [[ -n "${LUMINA_NOTARY_PROFILE:-}" ]]; then
    echo "Submitting with keychain profile $LUMINA_NOTARY_PROFILE"
    xcrun notarytool submit "$ZIP" --keychain-profile "$LUMINA_NOTARY_PROFILE" --wait
else
    : "${APPLE_ID:?APPLE_ID is required without LUMINA_NOTARY_PROFILE}"
    : "${APPLE_TEAM_ID:?APPLE_TEAM_ID is required}"
    : "${APPLE_APP_SPECIFIC_PASSWORD:?APPLE_APP_SPECIFIC_PASSWORD is required}"
    xcrun notarytool submit "$ZIP" \
        --apple-id "$APPLE_ID" \
        --team-id "$APPLE_TEAM_ID" \
        --password "$APPLE_APP_SPECIFIC_PASSWORD" \
        --wait
fi

xcrun stapler staple "$APP"
xcrun stapler validate "$APP"
spctl --assess --type execute --verbose=2 "$APP" || true
echo "Notarization complete"
