# Releasing Lumina

## Version bump

Keep these in sync (a release build fails on mismatch):

1. `crates/*/Cargo.toml` `version`
2. `resources/Info.plist` `CFBundleShortVersionString` / `CFBundleVersion`
3. `Makefile` `VERSION`

## Local unsigned build

```bash
make bundle                      # Lumina.app (universal if both rustup targets are installed)
make release                     # .zip + .dmg + SHA256SUMS
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # enable universal locally
```

Artifacts land in `target/release/bundle/osx/`. If the session cannot create
disk images (some sandboxes/headless CI), set `LUMINA_SKIP_DMG=1` to still
produce the zip and checksums; DMG creation is expected to work on a normal Mac
and in the release workflow.

## Signed build

```bash
LUMINA_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make release
```

The scripts sign the executable and bundle with the hardened runtime and
`resources/Lumina.entitlements`, then verify with `codesign --verify --deep --strict`.

## Notarization

Store credentials once (recommended):

```bash
xcrun notarytool store-credentials LuminaNotary \
  --apple-id "you@example.com" --team-id "TEAMID" --password "app-specific-password"
```

Then:

```bash
LUMINA_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
LUMINA_NOTARY_PROFILE=LuminaNotary \
make notarize
```

Alternatively set `APPLE_ID`, `APPLE_TEAM_ID`, and
`APPLE_APP_SPECIFIC_PASSWORD`. The app is submitted with `notarytool --wait`,
stapled, and validated. Packaging happens **after** notarization so the zip and
DMG contain the stapled app.

## Verification

```bash
codesign --verify --deep --strict --verbose=2 target/release/bundle/osx/Lumina.app
spctl --assess --type execute --verbose=2 target/release/bundle/osx/Lumina.app
xcrun stapler validate target/release/bundle/osx/Lumina.app
hdiutil verify target/release/bundle/osx/Lumina-v*.dmg
cd target/release/bundle/osx && shasum -a 256 -c SHA256SUMS.txt
```

## CI release

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml` on
`macos-14`, builds universal binaries, optionally signs/notarizes, uploads
artifacts, and creates a **draft** GitHub release with the zip, DMG, and
checksums. Publish the draft after verifying the artifacts.

### Repository secrets

| Secret | Purpose |
| --- | --- |
| `APPLE_CERTIFICATE_P12` | base64 Developer ID Application certificate (.p12) |
| `APPLE_CERTIFICATE_PASSWORD` | .p12 password |
| `KEYCHAIN_PASSWORD` | temporary CI keychain password |
| `APPLE_SIGN_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `LUMINA_NOTARY_PROFILE` | notarytool keychain profile name (or use the three below) |
| `APPLE_ID` | Apple ID for notarization |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `APPLE_APP_SPECIFIC_PASSWORD` | app-specific password for notarization |

Without signing secrets the workflow still builds ad-hoc artifacts and a draft
release, clearly unsigned — do not distribute those as a final release.
