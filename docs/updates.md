# Updates

Lumina checks for a newer release on macOS and Linux. Every installable update
must carry a manifest signed by the project's update key, and the downloaded
file's SHA-256 must match it. That signature is the trust anchor: it lets a
build update without Apple notarization, and it is the same check on every
platform.

| Platform | How an update arrives |
| --- | --- |
| macOS | Downloads the zip and swaps the app in when Lumina quits. |
| Linux | Never replaces root-owned or sandboxed files: Flatpak updates through Flathub; a `.deb` build points at the package manager and the release page. |
| Windows | Not implemented yet; the manifest carries Windows artifacts and the port will install them with MSIX/App Installer. |

## Using it

- **Check for Updates…**: the Lumina menu on macOS, the application menu on
  Linux, the command palette, and a button in Settings › About. It opens About,
  where the answer shows: up to date, downloading, ready, or what went wrong.
- **Check Daily** (Settings › About, or the palette): looks for an update when
  Lumina opens, at most once a day (a failed attempt counts, so an offline
  machine isn't retried at every launch). **Off by default.** A failed daily
  check stays quiet; a ready update, or one this copy can't install, shows a
  toast.
- On macOS, a ready update shows **Restart** and **Restart to Update**; if you
  choose neither, it installs whenever Lumina next quits and opens with your
  next launch. On Linux, About shows the new version and how to install it;
  nothing is replaced.

Nothing goes online unless you choose Check for Updates or turned on Check
Daily.

## What it does

1. **Find.** One HTTPS request to GitHub's "latest release" endpoint for
   `raysca/lumina`, with no user data (the user agent is just `Lumina`). That
   endpoint skips drafts and pre-releases, so publishing a release is what
   ships it. Only a plain `major.minor.patch` tag newer than the running
   version counts.
2. **Manifest.** Downloads `update.json` and `update.json.sig`. The manifest
   names the version, the release page, and one file with its SHA-256 for each
   platform. Its Ed25519 signature must verify against the public key compiled
   into the app, and the version must match the tag. A release without a valid
   manifest is not installable.
3. **Artifact.** Picks this platform's file from the manifest, downloads it
   over HTTPS, and checks its SHA-256 against the manifest. The manifest is
   verified before the download, so a damaged or swapped file never reaches
   step 4.
4. **Verify (macOS).** Unpacks the zip and requires a real `Lumina.app` folder
   whose code seal passes `codesign --verify --deep --strict`, with the
   `Info.plist` version the manifest names. When this Mac runs a Developer ID
   build, the new app must additionally meet the **running app's designated
   requirement and `notarized`** (same team and bundle id): a stolen update key
   alone cannot serve a differently signed app to a signed install. An ad-hoc
   or source build has no Developer ID to pin, so the manifest signature is its
   anchor.
5. **Install at quit (macOS).** The staged app is checked again (the staging
   folder is writable by any of your processes, so it may have changed since
   the download). Then the current app is renamed aside to
   `.Lumina-previous-<n>.app`, the new one moved into its place, and the old
   one deleted. If the second rename fails, the first is undone and the
   current version stays. Should even that fail, the old copy stays under its
   `.Lumina-previous-` name: cleanup only removes `.Lumina-update-` staging
   folders. After Restart to Update, a detached shell waits for Lumina to
   exit, then opens the new copy.

On Linux the files belong to the package manager, so steps 4 and 5 are not
attempted: About names the new version and whether Flathub or the Debian
package is the way to install it.

## Key custody

The private half of the update key exists only in the release secret store
(`UPDATE_SIGNING_KEY`); the public half is compiled into the app
(`crates/lumina-core/src/update/manifest.rs`). Losing the private key means a
build with a new public key must be installed once by hand before signed
updates can resume; a stolen key can sign updates for every platform, so it
deserves the same care as the Apple signing certificate. To rotate, ship a
build that trusts both keys, then drop the old one.

## When it won't install

It refuses, with a message and **Open Release Page** for a manual download,
when:

- the release has no manifest, or its signature or version doesn't match;
- the downloaded file's SHA-256 doesn't match the manifest;
- (macOS) it isn't a real app bundle, or it runs outside one (a `cargo run`
  build);
- (macOS) macOS runs it from a temporary quarantine location (never moved to
  Applications): move it, then check again;
- (macOS) it can't write to the folder the app is in;
- (macOS) the code seal, version, Developer ID requirement or notarization
  check fails.

Messages never name the download tools involved; details go to the log.

## Releases

For an update to install, a release must be **published** (not a draft),
tagged `v<major>.<minor>.<patch>`, and carry the platform files plus
`update.json` and `update.json.sig`. The release workflow builds the manifest
from the artifacts it publishes, so a release without the signing secret has no
manifest and cannot update installed copies.

- **macOS**: the zip may be ad-hoc or Developer ID signed. A Developer ID build
  additionally requires the new app to meet its team/notarization requirement;
  an ad-hoc build relies on the manifest key. An un-notarized build still needs
  the Gatekeeper **Open Anyway** steps in the release notes for its *first*
  install.
- **Linux**: a `.deb` is listed only when the Linux job signed its checksums;
  otherwise use the release page. Flatpak updates through Flathub.
- **Pre-releases** are skipped by the update endpoint. An ad-hoc preview can be
  published as a pre-release and still update from later normal releases.

Windows is planned with the port; until then no Windows artifacts are listed.
