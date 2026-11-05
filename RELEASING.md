# Releasing Lumina

## Supported platforms

| Platform | Status | Current artifacts |
| --- | --- | --- |
| macOS 12+ (arm64 + x86_64) | **Supported and released** | Universal `Lumina.app`, `.zip`, `.dmg` |
| Windows (x86_64; arm64 later) | **Planned, not supported yet** | None today |
| Linux (x86_64; arm64 later) | **Planned, not supported yet** | None today |

Only macOS is buildable and released from this tree today. Windows and Linux
have macOS-specific UI, player, decoder, packaging and CI paths to port first;
see [Windows and Linux (planned)](#windows-and-linux-planned). Do not publish,
document or support Windows/Linux artifacts until the acceptance criteria there
are met.

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

## Windows and Linux (planned)

These are future targets, not current capabilities. The macOS release path must
remain the primary, tested path while portability work lands incrementally.

### What blocks a Windows/Linux release today

- **UI platform integration** — the GPUI shell uses macOS titlebar/traffic-light
  options, AppKit/AVFoundation player bridging and macOS menu conventions.
  Non-macOS builds currently use a dummy player backend; the window, menu and
  input layers need platform-specific validation and, where GPUI lacks a
  portable API, small platform adapters.
- **Media decoding** — the JPEG/PNG/WebP path is portable, but HEIC/RAW
  thumbnails currently fall back to macOS `sips`/`qlmanage` and video
  thumbnails come from AVFoundation in process. Windows needs
  Windows Imaging Component (or libheif/ffmpeg) and Linux needs libheif/ffmpeg
  or GStreamer, with a capability matrix for unsupported formats.
- **Video playback** — the real player today is AVFoundation. Windows needs a
  Media Foundation backend; Linux needs GStreamer or ffmpeg. Frame timing,
  audio, scrubbing and PiP parity must be tested per platform.
- **Packaging and signing** — the release scripts use `codesign`,
  `notarytool`, `hdiutil`, `lipo` and `ditto`. Each platform needs its own
  build/package/sign pipeline and CI runner.
- **Live UI tests** — the OS-level harness is macOS/CoreGraphics/Swift based.
  In-process GPUI tests may be reusable, but each platform needs a compile/test
  job and a window smoke check.
- **Metadata and paths** — `Info.plist`, macOS config/cache conventions and
  `~/Library` paths need Windows/Linux equivalents (APPDATA/XDG) with migration
  and test coverage.

### Porting workstreams

1. **Compile/test matrix first**: `cargo check --workspace` and headless
   `lumina-core` tests on Windows and Linux CI. Gate the port on a green matrix
   before UI parity work.
2. **Core portability**: isolate OS backends behind traits; add portable decode
   fallbacks and document which formats are unsupported per platform.
3. **UI portability**: replace macOS-only window/menu/titlebar assumptions with
   platform adapters; make shortcut modifiers use the platform primary modifier
   (Cmd vs Ctrl); verify file dialogs, drag/drop and keyboard focus.
4. **Player backends**: implement Media Foundation (Windows) and
   GStreamer/ffmpeg (Linux), then run the existing viewer/video tests.
5. **Packaging and signing**:
   - Windows: build x86_64 (arm64 later), add icon/version resources, install as
     MSIX (or signed NSIS/MSI), sign with Authenticode and timestamping; plan
     SmartScreen reputation.
   - Linux: build x86_64 (arm64 later), ship AppImage plus deb/rpm (or Flatpak),
     add a desktop entry and icons, document glibc/distribution requirements,
     sign repository metadata/packages with GPG.
6. **CI/release matrix**: one job per platform, native tests, platform archive
   verification, per-platform checksums, and a single draft release containing
   macOS + Windows + Linux artifacts.
7. **QA and docs**: feature-parity matrix, support policy, installation and
   uninstall instructions, crash/log locations, and honest limitations.

### Target artifacts (not produced yet)

| Platform | Target formats | Signing |
| --- | --- | --- |
| Windows x86_64 | `Lumina-vX.Y.Z-windows-x64.msix` (or NSIS/MSI fallback), zip | Authenticode |
| Linux x86_64 | `Lumina-vX.Y.Z-linux-x64.AppImage`, `.deb`, `.rpm` | GPG repository/package signature |
| macOS universal | `Lumina-vX.Y.Z-macos-universal.zip`, `.dmg` | Developer ID + notarization |

### Acceptance criteria before calling Windows/Linux supported

- [ ] Native CI job builds, tests and produces a launchable artifact.
- [ ] Window smoke test opens, scans a library, opens the viewer and quits cleanly.
- [ ] Core catalog, search, smart albums, metadata editing, undo and XMP sidecars
      pass the existing test suite on that platform.
- [ ] Unsupported media formats fail gracefully and are documented.
- [ ] Installer/package installs and uninstalls without leftovers.
- [ ] Artifacts are signed and verified with the platform tooling; checksums are
      published; the release is not marked stable until all platforms pass.
- [ ] README, support policy and `RELEASING.md` no longer say macOS-only.

Until then, keep the release workflow macOS-only and do not upload partial
Windows/Linux builds to a public release.
