# Releasing Lumina

## Supported platforms

| Platform | Status | Current artifacts |
| --- | --- | --- |
| macOS 12+ (arm64 + x86_64) | **Supported** (no release tagged yet) | Universal `Lumina.app`, `.zip`, `.dmg` |
| Windows (x86_64; arm64 later) | **Planned, not supported yet** | None today |
| Linux (x86_64; arm64 later) | **In development, not supported yet** | Internal Ubuntu 24.04 amd64 `.deb` (glibc 2.39+); with the Linux release key set, the signed `.deb` also joins the draft release. Not a supported release |

Only macOS is supported from this tree. Linux now has a native GStreamer
player, ffmpeg video previews, a Debian packaging path and a native Linux CI
build/test job that passes on GitHub, but its window, install lifecycle and end-to-end library/viewer
flow have not completed the acceptance checks below. Windows remains planned.
Do not publish or support Linux artifacts until those checks pass.

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

### Ad-hoc preview release (no Developer ID)

The first public build can ship without an Apple Developer account:

```bash
make release        # no LUMINA_SIGN_IDENTITY: the scripts sign ad-hoc
```

Publish it **as a GitHub pre-release**. The updater asks `releases/latest`,
which skips pre-releases, so an ad-hoc build is never offered to an installed
app, and the release is unmistakably a preview rather than a supported version.

What users get:

- **First launch:** macOS cannot verify the developer. The notes must carry
  the **System Settings › Privacy & Security › Open Anyway** steps (macOS
  12–14: right-click Lumina › Open).
- **No identity to inspect:** `codesign -dv` shows `Signature=adhoc`; there is
  no Developer ID to verify, and `spctl` rejects it by design.
- **Updates come from the signed manifest.** An ad-hoc build verifies the
  update key like any other, so once a later normal release carries
  `update.json` About offers it. The preview is published as a pre-release, so
  it is never offered to an install that already exists; its own users still
  see later normal releases. The update key is the only authorship proof for
  these installs, so it belongs in the secret store, not the repository.

Start the release notes with:

> **Preview build.** Lumina is not signed with a Developer ID yet, so macOS
> will ask you to approve it. Open **System Settings › Privacy & Security**,
> click **Open Anyway** (macOS 12–14: right-click Lumina › Open), and confirm.
>
> **Updates:** this preview cannot update itself. Download future versions
> from the releases page; moving to the standard build needs one manual
> install.

The release workflow sets the pre-release flag for an ad-hoc draft. Do not
describe the build as signed or notarized; its updates are verified by the
project key, not by Apple.

### Signed without notarization

A Developer ID signature without an Apple notarization ticket is supported:

```bash
LUMINA_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" make release
# No LUMINA_NOTARIZE and no notary credentials.
```

Two consequences are user-visible and must be in the release notes:

- **Gatekeeper blocks the download on first launch.** The zip and DMG carry
  the quarantine flag, so macOS reports that it cannot check the app for
  malware. The user opens **System Settings › Privacy & Security**, clicks
  **Open Anyway** (on macOS 12–14: right-click the app › **Open**), and
  confirms. A signed-only build is not a double-click install.
- **The updater installs it through the signed manifest.** Apple notarization
  is not required for a signed-only install to update. A Developer ID install
  (one whose running app has a team) still requires a notarized new app, so a
  signed-only release cannot update those.

`spctl --assess` is a notarization check: on a signed-only build it reports
"rejected" by design and that is not a packaging failure. Verify the signature
instead.

Put this at the top of the release notes (adapt the version):

> **First launch:** Lumina is signed but not notarized by Apple yet. If macOS
> says it cannot verify the app, open **System Settings › Privacy & Security**,
> click **Open Anyway**, and confirm. On macOS 12–14, right-click Lumina and
> choose **Open**.
>
> **Updates:** Check for Updates downloads the release but cannot install it
> automatically until a notarized build ships; use **Open Release Page** and
> install the new version by hand.
>
> The zip's `SHA256SUMS.txt` is beside it if you want to verify the download.

## Notarization

Notarization is what makes a download open without the Gatekeeper prompt, and a
notarized new app is still required when updating a Developer ID install. The
updater itself trusts the signed manifest, so a signed-only or ad-hoc build can
update without it.

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

### Signed and notarized

```bash
codesign --verify --deep --strict --verbose=2 target/release/bundle/osx/Lumina.app
spctl --assess --type execute --verbose=2 target/release/bundle/osx/Lumina.app
xcrun stapler validate target/release/bundle/osx/Lumina.app
hdiutil verify target/release/bundle/osx/Lumina-v*.dmg
cd target/release/bundle/osx && shasum -a 256 -c SHA256SUMS.txt
```

### Signed only

```bash
codesign --verify --deep --strict --verbose=2 target/release/bundle/osx/Lumina.app
codesign -dv --verbose=4 target/release/bundle/osx/Lumina.app 2>&1 | grep -E "Authority|TeamIdentifier"
hdiutil verify target/release/bundle/osx/Lumina-v*.dmg
cd target/release/bundle/osx && shasum -a 256 -c SHA256SUMS.txt
```

`spctl --assess` and `xcrun stapler validate` belong to the notarized lane;
they fail on a signed-only build and that is expected.

## CI release

Pushing a tag such as `v0.2.0` runs `.github/workflows/release.yml` on
`macos-14`, builds universal binaries, optionally signs/notarizes, uploads
artifacts, and creates a **draft** GitHub release with the zip, DMG, and
checksums. Publish the draft after verifying the artifacts.

Publishing ships the release to every Lumina that checks for updates
(`docs/updates.md`). Installed apps take it only if the zip holds a
Developer ID signed, notarized and stapled `Lumina.app` from the same team
and bundle id, with the tag's version. Never publish an ad-hoc build as a
normal release; the documented ad-hoc preview below is the only exception.
Keep the zip and `SHA256SUMS.txt` names that `package_release.sh` writes.

A final job then builds `update.json` from the published artifacts, signs it
with `UPDATE_SIGNING_KEY`, and attaches it to the same draft. Every platform
verifies that manifest before installing an update; a release without it can
only be installed by hand.

The same tag builds the Linux package in the release workflow (after the
macOS job, so the draft exists). When `LINUX_GPG_PRIVATE_KEY` and
`LINUX_SIGNING_KEY` are set, it publishes `Lumina-vX.Y.Z-linux-amd64.deb`,
`SHA256SUMS-linux.txt`, `SHA256SUMS-linux.txt.asc` and the public key
`lumina-linux-signing-key.asc` on the **draft** release. Without them the
package stays a 30-day workflow artifact and is never attached: an unsigned
Linux artifact must not appear on a release. Linux is still not a supported
platform; see the acceptance criteria below.

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
| `UPDATE_SIGNING_KEY` | base64 of the update key's PEM; signs `update.json` for every platform |

Generate the update key once with `scripts/update_keygen.sh`; it prints the
public key to embed in `crates/lumina-core/src/update/manifest.rs` and leaves
the private half in a file only you can read. The secret is that file,
base64-encoded:

```bash
base64 < ~/.config/lumina-release/update-key.pem
```

Without it the release has no `update.json` and installed copies cannot update.

Linux release signing (optional; without it the `.deb` is not published):

| Secret | Purpose |
| --- | --- |
| `LINUX_GPG_PRIVATE_KEY` | base64 of the ASCII-armored private signing key |
| `LINUX_SIGNING_KEY` | fingerprint of that key (what `gpg --local-user` uses) |
| `LINUX_GPG_PASSPHRASE` | optional passphrase for the key |

Create a dedicated key, publish its fingerprint with the release, and keep the
private half only in the secret store:

```bash
gpg --quick-gen-key "Lumina Releases <releases@example.com>" default default never
gpg --armor --export-secret-keys <fingerprint> | base64 > key.b64
gpg --armor --export <fingerprint> > lumina-linux-signing-key.asc
```

Without signing secrets the workflow builds an ad-hoc draft and marks it as a
**pre-release**. Publish it only as the documented ad-hoc preview, with the
Gatekeeper and no-updates notes above — never as a normal release.

With a signing certificate but no notary credentials the draft is Developer ID
signed and not notarized. The workflow warns; that is a valid initial release
only with the Gatekeeper steps in the release notes, and its users update by
hand because the updater requires notarization.

## Windows planned; Linux in development

Linux development builds are not a supported release. Keep the macOS release
path as the only public release until Linux completes every acceptance check.
### CI build artifacts

CI builds Lumina for macOS and Linux on pushes to main and on pull requests,
and keeps two internal build artifacts for 14 days:

- `package-linux`: an unsigned amd64 `.deb`, built, installed, launched and
  removed by `scripts/check_linux_package.sh`, then uploaded as
  `lumina-linux-amd64-unsigned`.
- `bundle-macos`: an ad-hoc signed arm64 `Lumina.app` zip, uploaded as
  `lumina-macos-arm64-adhoc`.

The release workflow builds and checks the package too. With the Linux release
key set it attaches the signed `.deb`, its checksums and the public key to the
draft release; without it, the files stay a 30-day workflow artifact. An
unsigned Linux artifact is never attached to a release: it is not a
distributable package, and Linux is not a supported platform yet. The ad-hoc
macOS app on CI is likewise not Developer ID signed or notarized.

### What blocks a Windows/Linux release today

- **UI platform integration** — Linux now uses its native GPUI titlebar, but its
  menus, keyboard modifiers, file dialogs, drag/drop and window behavior still
  need real-session validation.
- **Media decoding** — Linux currently decodes JPEG/PNG/WebP in process and
  uses ffmpeg for video posters and scrub previews. HEIC/HEIF and RAW previews
  are unavailable there. Those errors and the supported media matrix need
  user-facing docs and real-library validation.
- **Video playback** — Linux now has a GStreamer player. Playback, audio output,
  frame timing, scrubbing, resume, looping, PiP and track-selection behavior
  still need parity testing. Windows needs a Media Foundation backend.
- **Packaging and signing** — the release scripts use `codesign`,
  `notarytool`, `hdiutil`, `lipo` and `ditto`. Each platform needs its own
  build/package/sign pipeline and CI runner. The Linux `.deb` is now versioned,
  checksummed and covered by a detached GPG signature on the checksums when the
  release key is configured; in-package signing is not used (`debsigs`
  0.01.19 implements signing but not verification), so a signed apt repository
  is still the next step for `apt`-based delivery.
- **Live UI tests** — Linux now has Xvfb smokes for X11 and nested Weston
  Wayland. The X11 check launches the app, scans an isolated H.264 video,
  displays its viewer frame and loads the GStreamer player; the Wayland check
  verifies the app creates a Wayland window. The workflow runs both on Ubuntu,
  and they pass in the hosted native x86_64 CI job, as does `package-linux`.
  Dialogs,
  drag/drop, audio output and other real-session interactions still need desktop
  validation (the pass is recorded with
  [`docs/linux-release-qa.md`](docs/linux-release-qa.md)). The separate
  CoreGraphics/Swift window probe remains macOS-only.
- **Metadata and paths** — Linux uses XDG paths. Startup migration and broader
  package upgrade/uninstall testing still need coverage.

### Porting workstreams

1. **Compile/test matrix**: Linux has native Ubuntu build/test/clippy CI, and
   it passes on GitHub along with the package job; keep both and the macOS job
   green. Windows still needs its own matrix.
2. **Core portability**: isolate OS backends behind traits; add portable decode
   fallbacks and document which formats are unsupported per platform.
3. **UI portability**: Linux now has a native titlebar, platform shortcut
   labels, an executable picker and a fixture-library/viewer smoke path. Verify
   file dialogs, drag/drop, keyboard focus and window behavior in real desktop
   sessions; keep macOS behavior green.
4. **Player backends**: run the viewer/video suite against Linux GStreamer and
   implement Media Foundation (Windows).
5. **Packaging and signing**:
   - Windows: build x86_64 (arm64 later), add icon/version resources, install as
     MSIX (or signed NSIS/MSI), sign with Authenticode and timestamping; plan
     SmartScreen reputation.
   - Linux: verify the current x86_64 `.deb` install/uninstall and desktop
     entry, document required GStreamer plugins and glibc support, and check the
     signed release checksums end to end; add a signed apt repository (or make
     Flathub the delivery path) before public distribution. arm64 and other
     package formats remain future work.
6. **CI/release matrix**: one job per platform, native tests, platform archive
   verification, per-platform checksums, and a single draft release containing
   macOS + Windows + Linux artifacts.
7. **QA and docs**: feature-parity matrix, support policy, installation and
   uninstall instructions, crash/log locations, and honest limitations.

### Target artifacts (not produced yet)

| Platform | Target formats | Signing |
| --- | --- | --- |
| Windows x86_64 | `Lumina-vX.Y.Z-windows-x64.msix` (or NSIS/MSI fallback), zip | Authenticode |
| Linux x86_64 | `Lumina-vX.Y.Z-linux-amd64.deb`, `SHA256SUMS-linux.txt` | Detached GPG signature on the checksums (`.asc`); not for public distribution yet |
| macOS universal | `Lumina-vX.Y.Z-macos-universal.zip`, `.dmg` | Developer ID + notarization |

### Acceptance criteria before calling Windows/Linux supported

- [ ] Native CI job passes formatting, clippy, core/UI tests and produces a launchable artifact.
      (Linux: passes on GitHub with a `.deb` that installs and launches;
      Windows: no job yet.)
- [ ] Window smoke test opens, scans a fixture library, opens media in the viewer and quits cleanly.
- [ ] Core catalog, search, smart albums, metadata editing, undo and XMP sidecars
      pass the existing test suite on that platform.
- [ ] Unsupported media formats fail gracefully and are documented.
- [ ] Installer/package installs and uninstalls without leftovers, including user data handling.
- [ ] Artifacts are signed and verified with the platform tooling; checksums are
      published; the release is not marked stable until all platforms pass.
- [ ] The manual desktop pass in [`docs/linux-release-qa.md`](docs/linux-release-qa.md)
      is recorded for the release (install, media, URL import, desktop
      integration, removable drive, Flatpak).
- [ ] README, support policy and `RELEASING.md` no longer say macOS-only.

Until then, keep public releases macOS-only. CI keeps the unsigned Linux
package (and the ad-hoc macOS app) as short-lived internal workflow artifacts
for testing; do not attach them to a public release.
