# Updates

Lumina updates itself on macOS: it downloads a new version, checks that
Apple signed it for the same developer, and swaps it in when you quit.

## Using it

- **Check for Updates…**: in the Lumina menu, in the command palette (⌘K),
  and as a button in Settings › About. It opens About, where the answer
  shows: up to date, downloading, ready, or what went wrong.
- **Check Daily** (Settings › About, or ⌘K "Check for Updates Daily"): looks
  for an update when Lumina opens, at most once a day (a failed attempt
  counts, so an offline Mac isn't retried at every launch). **Off by
  default.** A failed daily check stays quiet; a ready update, or one this
  copy can't install, shows a toast.
- When an update is ready, a toast offers **Restart**. About shows **Restart
  to Update**, and so does ⌘K. If you choose neither, the update installs
  whenever Lumina next quits (⌘Q, the Dock, logout or shutdown) and opens
  with your next launch. If Restart meets the running-downloads question and
  you cancel, nothing restarts; a later quit still installs.

Nothing goes online unless you choose Check for Updates or turned on Check
Daily.

## What it does

1. **Find.** One HTTPS request to GitHub's "latest release" endpoint for
   `raysca/lumina`, with no user data (the user agent is just `Lumina`). That
   endpoint skips drafts and pre-releases, so publishing a release is what
   ships it. Only a plain `major.minor.patch` tag newer than the running
   version counts.
2. **Download.** System `curl` (HTTPS only) fetches the release's
   `Lumina-v<version>-macos-universal.zip` (or the zip for this Mac's
   architecture) and `SHA256SUMS.txt` into a hidden `.Lumina-update-<n>`
   folder beside the app. A checksum mismatch means a damaged download.
3. **Verify.** The checksum comes from the same release as the zip, so it
   proves nothing about who made it. Trust comes from Apple code signing,
   as in Electron's macOS updater. The new app must:
   - be a real folder, not a link;
   - pass `codesign --verify --deep --strict` while meeting the **running
     app's designated requirement and `notarized`** (same Developer ID team
     and bundle id, and an Apple notarization ticket). The requirement checks
     notarization itself, so turning Gatekeeper off doesn't skip it;
   - have the version in its `Info.plist` that the release tag names.
4. **Install at quit.** The staged app is checked again (the staging folder
   is writable by any of your processes, so it may have changed since the
   download). Then the current app is renamed aside to
   `.Lumina-previous-<n>.app`, the new one moved into its place, and the old
   one deleted. If the second rename fails, the first is undone and the
   current version stays. Should even that fail, the old copy stays under
   its `.Lumina-previous-` name: cleanup only removes `.Lumina-update-`
   staging folders. After Restart to Update, a detached shell waits for
   Lumina to exit, then opens the new copy.

## When it won't install

It refuses, with a message and **Open Release Page** for a manual download,
when:

- the running copy isn't Developer ID signed (ad-hoc and local builds);
- it runs outside an app bundle (a `cargo run` build);
- macOS runs it from a temporary quarantine location (never moved to
  Applications): move it, then check again;
- it can't write to the folder the app is in;
- the release has no zip for this Mac, or the zip fails any check above.

Messages never name the tools involved; details go to the log.

## Releases

For an update to install, a release must be **published** (not a draft),
tagged `v<major>.<minor>.<patch>`, and carry the zip and `SHA256SUMS.txt`
that `scripts/package_release.sh` makes from a **Developer ID signed,
notarized and stapled** app with the same bundle id. Changing the signing
team or the bundle id breaks updates for existing installs: users then
update by hand once.

Linux has no releases, so the updater isn't offered there.
