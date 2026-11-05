# Signed and Notarized macOS Release (0.2.x) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Lumina's first real release: a Developer ID signed, notarized and stapled universal `Lumina.app` in a signed, notarized DMG and a zip, with checksums, release notes and a support policy, that installs and opens on a clean Mac without a Gatekeeper warning, and whose bundled helpers start under the hardened runtime.

**Architecture:** No app code changes. Fix the release scripts (two real bugs block `make notarize` today), sign every build with the hardened runtime so ad-hoc local bundles exercise the same runtime rules as releases, add one `scripts/verify_release.sh` that checks everything the notary service and Gatekeeper care about (and is run by `make release` and by CI), make `.github/workflows/release.yml` parse and refuse to draft an unsigned release from a tag, and add `CHANGELOG.md` and `SUPPORT.md`. Version sync becomes a `cargo test` so it fails on every PR, not only at release time.

**Tech Stack:** bash + Apple tools already used (`codesign`, `notarytool`, `stapler`, `spctl`, `hdiutil`, `lipo`, `ditto`, `vtool`/`otool`, `PlistBuddy`), GitHub Actions, Rust std for one test. No new crates, no new Actions.

## Global Constraints

- No app (`crates/**/src`) changes. The only Rust added is one std-only integration test.
- No update check, no network call from the app: ROADMAP.md:542 puts in-app updates at P2 0.7+ and ROADMAP.md:96 at 1.0. Out of scope here.
- Never print or commit credentials. Scripts read them from env or the notarytool keychain profile; CI from repository secrets only. `set -x` is forbidden in scripts that see secrets.
- A Gatekeeper, staple or signature failure must fail the script (no `|| true`).
- Release notes, README and SUPPORT copy describe features in plain words; they do not name yt-dlp, ffmpeg or curl (memory rule; AGENTS.md §5.4). THIRD_PARTY_NOTICES.md is the exception, it must name them.
- Do not claim "released", "notarized" or a macOS version as tested until it has been (AGENTS.md §15). Every unverified item below is marked **(needs credentials)** or **(needs hardware/permission)**.
- Stage explicit paths; no `git stash`; one logical commit per task.

## Review Focus

1. A nested helper (`Contents/MacOS/yt-dlp` or `ffmpeg`) without hardened runtime, secure timestamp or Developer ID authority: the notary service rejects the whole app. Caught by `verify_release.sh` check `check_macho_signatures` (Task 1), proven failing on today's ad-hoc bundle (no runtime flag) before Task 2. The check must accept `flags=0x10002(adhoc,runtime)` in dev mode; a literal `0x10000` match would fail every ad-hoc build forever.
8. A stray `Lumina-notarize.zip` (written by `notarize_release.sh:20-22` into the upload directory) published as a release asset without a checksum: Task 2 temp dir, Task 1 `check_checksums` listing rule.
9. A manual `workflow_dispatch` run creating a release from a branch: Task 4 tag-only release step.
2. The bundled downloader fails to start under the hardened runtime because library validation refuses its unpacked Python runtime: caught by `check_helpers_start` (Task 1), which runs both helpers from a copy extracted from the shipped zip, quarantined in distribution mode.
3. `make notarize` rejects a correctly Developer ID signed app because it greps `codesign -dv` output, which never contains `Authority=` lines (verified: `codesign -dv` on a Developer ID app prints 0 Authority lines, `-dvv` prints them). Regression: Task 2 runs `notarize_release.sh`'s guard against a Developer ID signed app **(needs credentials)**; without credentials, reviewer reproduces with `codesign -dv` vs `-dvv` on any third-party app in /Applications.
4. A tag `v0.2.1` pushed while Cargo/Info.plist/Makefile still say 0.2.0, or the version bumped in one file only: Task 3 test `bundle_versions_match_cargo` plus the tag check in release.yml (Task 4).
5. A tag pushed to a repo without signing secrets silently produces an ad-hoc draft release: Task 4 guard step `require-signing-on-tag`.
6. A downloaded DMG flagged by Gatekeeper on a clean Mac (unsigned DMG, no stapled ticket): Task 2 signs/notarizes/staples the DMG; Task 1 `check_dmg_gatekeeper`; Task 7 clean-Mac checklist **(needs hardware)**.
7. `LSMinimumSystemVersion` says 12.0 but a slice was linked for a newer macOS: Task 1 `check_min_os`.

---

## Current state (verified)

Release scripts and metadata:
- `scripts/build_release.sh:20-25` compares only `crates/lumina-ui-gpui/Cargo.toml` `version` to `CFBundleShortVersionString`. `CFBundleVersion`, `crates/lumina-core/Cargo.toml` and `Makefile:4` `VERSION := 0.2.0` are not checked, so AGENTS.md:18 ("release builds fail on mismatch") and AGENTS.md:356 are only partly true.
- `scripts/build_release.sh:69-75,92-97,106-118`: hardened runtime (`--options runtime`) and entitlements are applied only with a real identity. Ad-hoc bundles are signed with no runtime flag, so the hardened-runtime path (yt-dlp with `resources/yt-dlp.entitlements`) has never run on this machine or in CI.
- `scripts/build_release.sh:77-80,98-101` runs `yt-dlp --version` / `ffmpeg -version` after signing (good), but from the build tree, not from a downloaded, quarantined copy.
- `scripts/notarize_release.sh:15` guards with `codesign -dv "$APP" 2>&1 | grep -q "Developer ID Application"`. `codesign -dv` does not print `Authority=` lines (checked against `/Applications/Android Studio.app`: `-dv` 0 matches, `-dvv` shows `Authority=Developer ID Application: ...`). **`make notarize` therefore always exits with "App is not Developer ID signed".**
- `scripts/notarize_release.sh:40` `spctl --assess ... || true` swallows a Gatekeeper rejection.
- `scripts/package_release.sh:30-51` builds the DMG but never signs, notarizes or staples it; `RELEASING.md:75` tells users to `hdiutil verify` it only.
- `scripts/notarize_release.sh:20-22` writes `Lumina-notarize.zip` into `target/release/bundle/osx` and never deletes it; `release.yml:65,75` upload `target/release/bundle/osx/*.zip`, so a notarized CI run would attach this unlisted zip (not in SHA256SUMS) to the release.
- Entitlements: `resources/Lumina.entitlements:6-7` (only `allow-jit` false, no exceptions); `resources/yt-dlp.entitlements:9-10` (`disable-library-validation`). ffmpeg is signed with no entitlements (`build_release.sh:93-94`). No `get-task-allow` anywhere.
- Measured 2026-11-05 on a scratch copy of `target/deps/yt-dlp` (2026.10.26, arm64 host): signed ad-hoc with `--options runtime` and `yt-dlp.entitlements`, `--version` and `--list-impersonate-targets` (imports the bundled curl_cffi, offline) both exit 0; signed with `--options runtime` and **no** entitlements, it dies with `Failed to load Python shared library ... mapping process and mapped file (non-platform) have different Team IDs`. So `disable-library-validation` is needed, is sufficient for those paths, and library validation is enforced on ad-hoc builds too (Decision 1 really tests it). `allow-unsigned-executable-memory` was not needed for these paths; an actual download was not exercised. ffmpeg ad-hoc + runtime runs. The x86_64 slices could not be run (Rosetta not installed here).
- Helper slices: `target/deps/yt-dlp` and `ffmpeg` are universal (`x86_64 arm64`). yt-dlp's arm64 slice has `LC_BUILD_VERSION minos 11.0`, its **x86_64 slice has no `LC_BUILD_VERSION`, only `LC_VERSION_MIN_MACOSX version 10.13`** (so `vtool -show-build` prints nothing for it); ffmpeg is 12.0 on both (`fetch_ffmpeg.sh:14`); the arm64 main binary is 11.0. The current local bundle is arm64-only.
- Ad-hoc `codesign -dvv` prints `flags=0x2(adhoc)`; ad-hoc + runtime prints `flags=0x10002(adhoc,runtime)`, Developer ID + runtime `flags=0x10000(runtime)`. A check must look for `runtime` inside the flag list, not a literal flags value.
- `resources/Info.plist:12` bundle id `com.lumina.photos`; `:22,24` both versions 0.2.0; `:26` `LSMinimumSystemVersion` 12.0; `:34` copyright "Copyright © 2026 Lumina. All rights reserved." while the project is MIT (`LICENSE:3` "Copyright (c) 2026 Lumina contributors", `README.md:212`, `crates/lumina-ui-gpui/Cargo.toml:7`).
- `RELEASING.md:51-54` tells the owner to pass the app-specific password on the `notarytool store-credentials` command line (ends up in shell history).
- System tools spawned by bare name: `curl` (`net_import/direct.rs:81,150`, `net_import/hls.rs:14`), `sips` and `qlmanage` (`decoder.rs:283,325`), `mdls` (`scanner.rs:679`), `open` (`app/preferences.rs:88`, `app/viewer_actions.rs:70,538,587`). These are Apple-signed binaries outside the bundle; the hardened runtime does not restrict spawning them and they are not part of notarization. No change needed.
- Bundled helpers are found beside the executable first (`net_import/helper.rs:25-43`); the env override is debug-only (`helper.rs:49`), so a release app cannot be pointed at another binary.

CI:
- `.github/workflows/release.yml:28` uses `if: ${{ secrets.APPLE_CERTIFICATE_P12 != '' }}`. GitHub documents that `secrets` cannot be referenced in step `if:` conditionals; the workflow is expected to fail validation. Not confirmed by a run: **the repo has no git remote** (`git remote -v` is empty, `gh run list` fails), so neither workflow has ever run on GitHub, and `git tag` is empty (no release has ever been cut).
- `release.yml:48-49` passes `LUMINA_NOTARY_PROFILE`, which names a keychain profile that cannot exist on a fresh runner; only the Apple ID path (`release.yml:50-52`) can work in CI.
- `release.yml` runs no tests, no tag/version check, no verification beyond `codesign --verify`; `:73` uses `generate_release_notes` (commit list), not curated notes; `:14` `macos-14` while `ci.yml:18` uses `macos-latest`.
- `release.yml:69-77` runs `softprops/action-gh-release` unconditionally, including on `workflow_dispatch` from a branch, where there is no tag (the action falls back to the ref name). Only tag runs should create a release.
- `secrets` in `jobs.<id>.env` and step `env:` is allowed (the current `:30-32,47-52` are valid); only step/job `if:` cannot read `secrets`.
- `ci.yml:41-45` runs `live_ui_smoke.sh` and `live_ui_automation.sh`, which SKIP without WindowServer/Accessibility (`live_ui_automation.sh:31-33,270-281`).

E2E and permissions:
- `scripts/live_ui_automation.sh:18,21` takes `LUMINA_UI_BIN` and `LUMINA_REQUIRE_UI_PERMISSIONS`; with the latter set, missing PostEvent permission fails (`:277-278`). It can already target the bundled release binary; no script change needed. Without the flag it prints `PASS: live UI automation (interaction tests skipped)` (`:280`), so Task 7 must always set it.
- Local machine: `rustup target list --installed` has `aarch64-apple-darwin` but not `x86_64-apple-darwin` (local `make release` builds arm64 only, `build_release.sh:42-45`); `security find-identity -v -p codesigning` lists one "Apple Development" identity and **no "Developer ID Application" identity**.

Docs:
- No `CHANGELOG.md`, `SUPPORT.md` or `SECURITY.md`. `RELEASING.md:7` says macOS is "**Supported and released**" (no release exists); `RELEASING.md:99-100` promises an ad-hoc draft release without secrets; `:82` names `macos-14`. `ROADMAP.md:55-56,427-429` correctly say the pipeline is unexercised. `ROADMAP.md:89` scopes 0.2.x to signed release, CI validated, e2e permission run, release notes and support docs.
- `THIRD_PARTY_NOTICES.md:54,56` hard-codes ffmpeg 9.0.2, which must match `scripts/fetch_ffmpeg.sh:12`.
- Log file: `~/Library/Application Support/lumina/logs/lumina.log` (`crates/lumina-ui-gpui/src/logging.rs:54-61`); data/cache paths `README.md:88-91`.
- `crates/lumina-ui-gpui` is bin-only (`Cargo.toml:9-16`, no `[lib]`); `tests/` holds only `fixtures/`. A std-only integration test there compiles without importing the crate.

## Decisions

1. **Sign every build with the hardened runtime and entitlements; only `--timestamp` depends on a real identity.** Ad-hoc + runtime runs locally, so every `make bundle` tests library validation for the downloader. Rejected: keep the ad-hoc branch runtime-free (today): the riskiest runtime rule would first be exercised by the notary service. This also deletes three if/else branches. Measured: library validation is enforced on ad-hoc + runtime builds (Current state), so this is a real test, not a formality. Cost: a debugger cannot attach to a bundled build (no `get-task-allow`); `make dev`/`make run-release` run the unbundled cargo binary and are unaffected. Never sign with `--deep` (Apple discourages it; it would also stamp the app's entitlements onto helpers); `--deep` stays only in `codesign --verify`.
2. **Sign, notarize and staple the DMG as well as the app** (two notary submissions: the app as a zip built in a temp dir, the DMG submitted as-is). Rejected: notarize only the DMG and staple the app from its ticket (one submission, but reorders packaging and the zip would need the DMG first); rejected leaving the DMG unsigned: it cannot be stapled and Gatekeeper assesses it on open.
3. **One verification script, `scripts/verify_release.sh`, with a dev mode (ad-hoc) and a distribution mode (Developer ID).** `package_release.sh` and release.yml call it. Rejected: inline checks in each script (they already drift: the `|| true`), or a Rust test (needs a built bundle, not a unit-test concern).
4. **Version sync as a std-only `cargo test`** in `crates/lumina-ui-gpui/tests/release_metadata.rs`, so a mismatch fails `make test` and CI on every PR. Release.yml runs `cargo test --workspace` and adds only the tag check. Rejected: extend the shell check in `build_release.sh` (runs only at release time) or a separate `check_versions.sh` (a second source of truth).
5. **CI notarizes with Apple ID + app-specific password** (already implemented). Drop `LUMINA_NOTARY_PROFILE` from release.yml and the secrets table. Rejected now: App Store Connect API key (`notarytool --key`), better for teams; see open question 3.
6. **A tag push requires signing and notarization secrets; `workflow_dispatch` may still build ad-hoc drafts.** Rejected: the current silent fallback.
7. **Curated `CHANGELOG.md` (Keep a Changelog headings), release body taken from its section for the tag.** Rejected: GitHub's generated notes (commit subjects, tool names, no user framing).
8. **`SUPPORT.md` at the repo root** (GitHub links it from the issue form). Rejected: `docs/support.md` (less discoverable).
9. **No update check in this milestone** (ROADMAP.md:542, 0.7+). The support policy tells users how to learn about a new release (the releases page) instead.
10. **No reproducible-build work.** Signatures embed timestamps, so the bytes cannot be reproduced; we publish SHA256SUMS of what we ship. Grade A item 10 ("reproducible checksums", ROADMAP.md:82) is read as "checksums that verify", stated honestly in RELEASING.md.
11. **Release runner pinned to one image for both workflows** (`macos-15`), so CI tests what release builds. Availability of the image at release time must be checked; it is not verifiable from here.

---

## Tasks

### Task 1: `scripts/verify_release.sh` (fails on today's bundle)

**Files:**
- Create: `scripts/verify_release.sh`
- Modify: `Makefile` (`verify-release` target, add to `.PHONY`), `RELEASING.md` (Verification section)
- Not modified here: `scripts/package_release.sh`. Wiring the script into packaging is Task 2, because today's bundle must fail it (otherwise `make release` would break between the two commits).

**Interfaces:**
- `scripts/verify_release.sh [OUT_DIR]` (default `target/release/bundle/osx`). Mode: distribution when the app's `codesign -dvv` shows `Authority=Developer ID Application`, or forced with `LUMINA_REQUIRE_DISTRIBUTION=1` (CI on tags); otherwise dev. Exit 0 = all checks pass, 1 = first failure with `FAIL: <check>: <reason>`. Prints `PASS:`/`SKIP:` per check; never prints secrets.
- Checks (shell functions, names are the review contract):
  - `check_bundle_seal`: `codesign --verify --deep --strict --verbose=2` on the app.
  - `check_macho_signatures`: for **every** file in `Contents/MacOS` (fail on a file that is not Mach-O, so a future helper cannot slip through unchecked): the `flags=` list in `codesign -dvv` contains `runtime` (match `flags=0x[0-9a-f]+\([^)]*runtime`, since ad-hoc prints `flags=0x10002(adhoc,runtime)`); distribution mode also requires `Authority=Developer ID Application`, the same `TeamIdentifier` as the app, a `Timestamp=` line, and no `adhoc` flag.
  - `check_entitlements`: `codesign -d --entitlements :- ` per binary. App: no key with value `true`, no `get-task-allow`. yt-dlp: exactly the keys of `resources/yt-dlp.entitlements` (today only `com.apple.security.cs.disable-library-validation`; compare against the file so the script and the file cannot drift). ffmpeg: none. Any other file in `Contents/MacOS`: none.
  - `check_archs`: `lipo -archs` of each helper contains every arch of the main binary (a universal app with an arm64-only helper fails on Intel).
  - `check_min_os`: per slice of every Mach-O, the deployment target is <= `LSMinimumSystemVersion`. Read `LC_BUILD_VERSION minos` (`vtool -show-build`) **or** `LC_VERSION_MIN_MACOSX version` (`otool -arch <a> -l`): yt-dlp's x86_64 slice has only the latter (10.13), and `vtool -show-build` prints nothing for it. A slice with neither load command fails.
  - `check_resources`: `LICENSE`, `THIRD_PARTY_NOTICES.md`, `ffmpeg-LICENSE.txt`, `yt-dlp-THIRD_PARTY_LICENSES.txt`, `ffmpeg-source/ffmpeg-<ver>.tar.xz` and `ffmpeg-source/fetch_ffmpeg.sh` (`build_release.sh:90-91`; LGPL build script) exist in `Contents/Resources`; `<ver>` read from `scripts/fetch_ffmpeg.sh`.
  - `check_helpers_start`: `ditto -x -k` the shipped zip into a `mktemp -d`; in distribution mode first set `com.apple.quarantine` on the zip copy and extract with `ditto --qtn`, and require `spctl --assess --type execute` and `xcrun stapler validate` on the extracted app (what a user actually gets); then run `yt-dlp --ignore-config --version`, `yt-dlp --ignore-config --list-impersonate-targets` (loads the bundled native networking library offline; verified to work ad-hoc + runtime) and `ffmpeg -hide_banner -version` from the extracted bundle with a 30 s timeout each (no `timeout` binary on stock macOS: background + `kill`, or `perl -e 'alarm 30; exec @ARGV'`). Runs only the host arch's slice; the other slice is covered by Task 7. Cleans up on exit (trap).
  - `check_staple` (distribution only): `xcrun stapler validate` on the app in OUT_DIR and on the DMG.
  - `check_dmg_gatekeeper`: no DMG: SKIP in dev mode, FAIL in distribution mode. Otherwise `hdiutil verify`; distribution also `codesign --verify` on the DMG, `spctl --assess --type open --context context:primary-signature -v` on it, and `hdiutil attach -nobrowse -readonly -mountpoint <mktemp>` to check the app inside is stapled and passes `codesign --verify --strict` (detach in the trap).
  - `check_checksums`: `shasum -a 256 -c SHA256SUMS.txt` in OUT_DIR, and every `*.zip`/`*.dmg` in OUT_DIR is listed in it (catches a stray upload such as today's `Lumina-notarize.zip`).
- `make verify-release` runs it.

**Tests (script layer; no Rust):**
- [ ] On the current ad-hoc bundle (`make release` with `LUMINA_SKIP_DMG` unset, then `make verify-release`), the script must FAIL at `check_macho_signatures` (`flags=0x2(adhoc)`, no runtime, `build_release.sh:74`). Record the output in the commit body. This is the regression proof for Task 2.
- [ ] Positive self-check without Task 2: on a scratch copy of the bundle, re-sign the three binaries and the bundle ad-hoc with `--options runtime` (yt-dlp with its entitlements) and rebuild the zip and SHA256SUMS there; the script passes in dev mode. Proves the `runtime` regex accepts `flags=0x10002(adhoc,runtime)`.
- [ ] Negative self-checks, run by the author on copies and listed in the commit body: strip the helper signature (`codesign --remove-signature`) -> `check_macho_signatures` fails; re-sign yt-dlp with runtime but without entitlements -> `check_entitlements` fails (and `check_helpers_start` would fail with the "different Team IDs" error); drop a non-Mach-O file into `Contents/MacOS` -> `check_macho_signatures` fails; delete `ffmpeg-LICENSE.txt` -> `check_resources` fails; edit one byte of the zip -> `check_checksums` fails; add an unlisted `extra.zip` to OUT_DIR -> `check_checksums` fails.
- [ ] `check_min_os` on today's helpers reports yt-dlp x86_64 10.13 and arm64 11.0 (not "unknown"); with `LSMinimumSystemVersion` edited to 10.12 on a copy it fails.
- [ ] `bash -n` and `shellcheck` if installed (state if not installed; do not add it as a dependency).

Docs: `RELEASING.md` Verification section points to `make verify-release` (replace the loose command list at RELEASING.md:69-77). Size: M.

### Task 2: Fix signing and notarization in the release scripts

**Files:**
- Modify: `scripts/build_release.sh`, `scripts/notarize_release.sh`, `scripts/package_release.sh`, `RELEASING.md`

**Interfaces / behavior:**
- `build_release.sh`: one `sign()` helper used for yt-dlp, ffmpeg, main binary and bundle: `codesign --force --options runtime [--timestamp when SIGN_IDENTITY != "-"] [--entitlements FILE] --sign "$SIGN_IDENTITY" PATH`. Inside-out order unchanged (helpers, then main binary, then bundle). No `--deep` when signing (Decision 1). The post-sign `--version` checks stay.
- `notarize_release.sh`: take the app or DMG path as `$1` (default the app). Guard uses `codesign -dvv` (fixes the always-fail at :15) and prints `Developer ID signature found` when it passes. Check credentials (profile or the three Apple ID variables) right after the guard, before any zipping. An app is zipped with `ditto -c -k --keepParent` into a `mktemp -d` that a trap removes (fixes the stray `Lumina-notarize.zip` that `release.yml` would upload); a DMG is submitted as-is. Submit with `notarytool submit --wait --timeout 30m --output-format json`, capturing output and exit code without `set -e` aborting (`out=$(...) || rc=$?`, not `|| true`); read `id` and `status` with `plutil -extract <key> raw -o - -` (plutil reads JSON; no python/jq dependency). On status other than `Accepted` or a non-zero exit, run `notarytool log <id>` (same credentials) when an id exists, and exit 1, so the rejection reason is in the log. Staple and `stapler validate` the given path. `spctl` failure is fatal (remove `|| true` at :40); use `--type execute` for the app and `--type open --context context:primary-signature` for a DMG.
- `package_release.sh`: after creating the DMG, when `LUMINA_NOTARIZE=1`: `codesign --timestamp --sign "$LUMINA_SIGN_IDENTITY" "$DMG"`, then `notarize_release.sh "$DMG"`; checksums are computed after stapling; then `verify_release.sh` (always, also ad-hoc; this is where Task 1's script is wired in). When `LUMINA_NOTARIZE=1` and `LUMINA_SKIP_DMG=1`, refuse (a notarized release always has a DMG).

**Tests:**
- [ ] `make release` (ad-hoc) now passes `scripts/verify_release.sh` in dev mode, including `check_macho_signatures` and `check_helpers_start`. This proves the downloader starts under hardened runtime with library validation disabled, the first time it has been exercised. Expected to pass: the same signing was reproduced on a scratch copy (Current state). If yt-dlp fails to start, do NOT add more entitlements by guesswork: capture its stderr (PyInstaller prints the dlopen/code-signature reason, as in the measured failure) and `log show --last 5m --predicate 'subsystem == "com.apple.securityd"'`, and stop for review.
- [ ] Guard regression without credentials: `scripts/notarize_release.sh "/Applications/<any third-party Developer ID app>"` with no notary env prints `Developer ID signature found` and then stops at the missing-credentials message (before zipping). The old guard expression (`codesign -dv <same app> 2>&1 | grep -q "Developer ID Application"`) returns 1 on the same app, which is the bug. `scripts/notarize_release.sh target/release/bundle/osx/Lumina.app` (ad-hoc) still refuses. Record both in the commit body.
- [ ] After `make release`, `ls target/release/bundle/osx/*.zip` lists only the versioned zip.
- [ ] Optional pre-credential spike, owner machine: sign with the existing "Apple Development" identity (`LUMINA_SIGN_IDENTITY="Apple Development: ..." LUMINA_NOTARIZE=0 make release`) to exercise `--timestamp` against Apple's timestamp server. Expected: build passes, `notarize_release.sh` refuses (not Developer ID). Proves the guard both ways.
- [ ] With a Developer ID identity **(needs credentials)**: `make notarize` reaches `Accepted` for both app and DMG; `verify_release.sh` passes in distribution mode.

Docs: RELEASING.md "Signed build" (every build, ad-hoc included, uses the hardened runtime; RELEASING.md:44-45 says so only for Developer ID) and "Notarization" (DMG is now signed/notarized/stapled; how to read a rejection log; drop `--password "app-specific-password"` from the `store-credentials` example at RELEASING.md:51-54 so it prompts instead). Size: M.

### Task 3: Version sync as a test

**Files:**
- Create: `crates/lumina-ui-gpui/tests/release_metadata.rs`
- Modify: `AGENTS.md:18` and `:356` only if wording must change (it becomes true; likely no change)

**Interfaces:** none (test only). Reads repo files via `env!("CARGO_MANIFEST_DIR")/../..`, std only, simple line scanning (no plist crate).

**Tests (integration layer, `cargo test -p lumina-ui-gpui --test release_metadata`):**
- [ ] `bundle_versions_match_cargo`: `CFBundleShortVersionString`, `CFBundleVersion` (resources/Info.plist), `crates/lumina-core/Cargo.toml` version and `Makefile` `VERSION` all equal `env!("CARGO_PKG_VERSION")`. Message: "version X in <file> must match Cargo 0.2.0; bump all files together (RELEASING.md Version bump)".
- [ ] `notices_name_the_pinned_ffmpeg`: `FFMPEG_VERSION` in `scripts/fetch_ffmpeg.sh` appears in THIRD_PARTY_NOTICES.md's FFmpeg section (catches a bumped tarball with stale LGPL notice text).
- [ ] `changelog_has_current_version` (added in Task 5's commit, once CHANGELOG.md exists): a `## [<version>]` heading exists.
- [ ] Regression proof: change `CFBundleVersion` to 0.2.9 locally, test fails; revert. State in the commit body.

Note `#[test]` is safe here (no `gpui::*` glob in an integration test file). The crate is bin-only, so the test cannot and need not import it; it still links the crate's dependencies at build time, which `cargo test --workspace` already builds. Size: S.

### Task 4: Make the release workflow valid and strict

**Files:**
- Modify: `.github/workflows/release.yml`, `.github/workflows/ci.yml` (runner image only), `RELEASING.md` (secrets table)

**Behavior:**
- Job-level `env` (the `secrets` context is allowed there, not in `if:`): `HAS_CERT: ${{ secrets.APPLE_CERTIFICATE_P12 != '' && secrets.APPLE_CERTIFICATE_PASSWORD != '' && secrets.KEYCHAIN_PASSWORD != '' && secrets.APPLE_SIGN_IDENTITY != '' }}`, `HAS_NOTARY: ${{ secrets.APPLE_ID != '' && secrets.APPLE_TEAM_ID != '' && secrets.APPLE_APP_SPECIFIC_PASSWORD != '' }}`. Only the booleans go in job env; the secret values stay in the env of the steps that use them. The import step uses `if: env.HAS_CERT == 'true'` (replaces the invalid `secrets` in `if:` at :28).
- Step `require-signing-on-tag` (first step): on `refs/tags/v*`, fail unless `HAS_CERT` and `HAS_NOTARY` are true. To name the missing secrets (names only) the step maps each secret to an env var and tests `-z` in bash; it never echoes values.
- When `HAS_CERT` is false, `LUMINA_SIGN_IDENTITY` stays `-` even if `APPLE_SIGN_IDENTITY` alone is set (today `:54-57` would try to sign with an identity whose certificate was never imported).
- Step `check-tag-version`: on tags, `GITHUB_REF_NAME` must equal `v` + the version from `crates/lumina-ui-gpui/Cargo.toml`.
- Step `cargo test --workspace` before packaging (versions, notices, changelog tests included).
- Packaging env: `LUMINA_NOTARIZE: ${{ env.HAS_NOTARY == 'true' && '1' || '0' }}`; remove `LUMINA_NOTARY_PROFILE`; set `LUMINA_REQUIRE_DISTRIBUTION=1` on tags so `verify_release.sh` runs in distribution mode.
- Release body: `awk` the `## [<version>]` section of CHANGELOG.md into `$RUNNER_TEMP/notes.md` and fail if it is empty; `softprops/action-gh-release@v2` with `body_path`, `draft: true`, and drop `generate_release_notes`.
- The release step (and the notes step) run only on tags: `if: startsWith(github.ref, 'refs/tags/v')`. `workflow_dispatch` uploads artifacts only (`actions/upload-artifact`), no release, so an ad-hoc build can never become a release draft. RELEASING.md:99-100 changes accordingly.
- Optional (open question 4): `environment: release` on the job so secrets need an approval.
- `runs-on: macos-15` in both workflows (Decision 11).

**Tests:** Workflows cannot be exercised from this machine (no remote).
- [ ] If the owner installs `actionlint` (a dev tool, not a project dependency), run it on both workflows; otherwise say it was not run.
- [ ] Reviewer reads the YAML against GitHub's context-availability table: no `secrets.` inside any `if:`; `secrets.` appears only in job/step `env:` and `with:`.
- [ ] After the remote exists: `workflow_dispatch` run without secrets -> ad-hoc artifact uploaded, verify passes in dev mode, **no** release or tag created. Then push a throwaway tag `v0.0.0-test` -> `require-signing-on-tag` fails if secrets are missing, `check-tag-version` fails otherwise (proves both guards); delete the tag and the draft. **(needs GitHub repo)**

Docs: RELEASING.md "CI release" + secrets table (drop `LUMINA_NOTARY_PROFILE`; `macos-14` at :82 becomes the pinned image; explain HAS_* behavior; replace :99-100 with "without secrets, tags fail and manual runs upload unsigned artifacts only"; note that the .p12 must contain the private key). Size: M.

### Task 5: CHANGELOG and release notes

**Files:**
- Create: `CHANGELOG.md`
- Modify: `crates/lumina-ui-gpui/tests/release_metadata.rs` (add `changelog_has_current_version`), `RELEASING.md` (release checklist: move Unreleased to the version heading with date before tagging)

**Content:** `## [Unreleased]` and `## [0.2.0] - YYYY-MM-DD` (date set in Task 7) with Added / Fixed / Known limitations, written for users, drafted from `git log` (169 commits) and the shipped list in ROADMAP.md:42-50 corrected by the code (copy-import with drag and drop and duplicate skipping, video resume, toasts, v3 viewer/player/tag manager, in-process video thumbnails, import video from a URL). Known limitations copied in substance from AGENTS.md §15 (VoiceOver sees little beyond the window, macOS only, no semantic search). No tool names. Link `SUPPORT.md` and `THIRD_PARTY_NOTICES.md`.

**Tests:** `changelog_has_current_version` (Task 3 file). Size: S.

### Task 6: Support policy, install docs, honest status

**Files:**
- Create: `SUPPORT.md`
- Modify: `README.md` (new "Install" section above "Requirements": download DMG from the releases page, drag to Applications, verify with `shasum -a 256 -c SHA256SUMS.txt` and `spctl --assess -v /Applications/Lumina.app`), `RELEASING.md:7` (status "Supported; first signed release pending" until Task 7, then "released"), `ROADMAP.md:55-56,89,427-433` (only after Task 7), `resources/Info.plist:34` `NSHumanReadableCopyright` -> "Copyright © 2026 Lumina contributors. MIT License." (holder taken from `LICENSE:3` and `README.md:212`; shown in Finder Get Info and the About panel)

**SUPPORT.md content:** supported: latest release only, macOS 12+ on Apple silicon and Intel (state which macOS versions were actually tested, from Task 7); unsupported: Windows/Linux, betas of macOS until tested; what to include in a report (version from Preferences, `crates/lumina-ui-gpui/src/views/preferences.rs:333`; macOS version; log at `~/Library/Application Support/lumina/logs/lumina.log`, reviewed for paths before sharing); privacy (no telemetry, network only for URLs the user imports, AGENTS.md §5.4); where data lives and how to uninstall (app + `~/Library/Application Support/lumina` + `~/Library/Caches/lumina`, warn that the first deletes the catalog but never the photos); how to verify a download; how to learn about new releases (watch the releases page; no in-app check yet, planned later). Issue link depends on open question 1.

**Tests:** docs only, plus `bundle_versions_match_cargo` still passing after the plist edit (it reads the same file; `plutil -lint resources/Info.plist` must pass). Reviewer checks every path and claim against the cited code. Size: S.

### Task 7: Cut v0.2.0 (owner-run checklist, no code)

**Owner prerequisites (exactly what must exist):**
- [ ] Apple Developer Program membership (paid) and its Team ID. The current machine only has an "Apple Development" identity, which cannot be notarized for distribution.
- [ ] A **Developer ID Application** certificate created by the Account Holder (developer.apple.com > Certificates), installed with its private key in the login keychain; `security find-identity -v -p codesigning` lists it.
- [ ] Notarization credentials: app-specific password (appleid.apple.com) for the Apple ID, then once locally `xcrun notarytool store-credentials LuminaNotary --apple-id ... --team-id ...` (prompts for the password; it is not put on the command line in shell history).
- [ ] `rustup target add x86_64-apple-darwin` (missing locally) for a universal build.
- [ ] A GitHub repository and remote (none configured), with secrets `APPLE_CERTIFICATE_P12` (base64 of an exported .p12 including the private key), `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD` (any random string), `APPLE_SIGN_IDENTITY` (full identity string), `APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_SPECIFIC_PASSWORD`.
- [ ] Accessibility and Screen Recording granted to the terminal app that runs the e2e script (System Settings > Privacy & Security).
- [ ] A clean test Mac: a fresh macOS user account at minimum, ideally a second Mac or VM on macOS 12 to back the minimum-version claim, and one Intel Mac for the x86_64 slices (app, yt-dlp, ffmpeg). Fallback if no Intel Mac: `softwareupdate --install-rosetta`, then `arch -x86_64` runs of the three binaries from the stapled app; state in SUPPORT.md that Intel was checked under Rosetta only. Rosetta is not installed on the current machine (`arch -x86_64` fails with "Bad CPU type").

**Run order:**
- [ ] Full gate: `cargo fmt --all -- --check`, clippy `-D warnings`, `cargo test --workspace`, `make bench` (serialized), `make ui-smoke`.
- [ ] "CI validated" (ROADMAP.md:89): after pushing to the new remote, `ci.yml` is green on `main` on GitHub for the commit to be tagged, all three jobs; record the run URL. **(needs GitHub repo)**
- [ ] `LUMINA_SIGN_IDENTITY="Developer ID Application: ... (TEAMID)" LUMINA_NOTARY_PROFILE=LuminaNotary make notarize`; `make verify-release` in distribution mode passes.
- [ ] `LUMINA_UI_BIN=target/release/bundle/osx/Lumina.app/Contents/MacOS/lumina-ui-gpui LUMINA_REQUIRE_UI_PERMISSIONS=1 make ui-e2e` (signed, hardened release binary; any SKIP is a failure here). Then the same against the debug binary. Record both outputs.
- [ ] Clean Mac: download the DMG through a browser (so it is quarantined), open it, drag to Applications, first launch shows only the standard "downloaded from the internet" prompt naming a verified developer; choose a library in Pictures and one on Desktop (folder-access prompt appears and is accepted); play a video; import one video from a URL (exercises the bundled helpers under Gatekeeper); quit; relaunch; `spctl --assess -v` reports "Notarized Developer ID".
- [ ] Date the CHANGELOG section, update RELEASING/ROADMAP status (Task 6 follow-up), commit `docs: release 0.2.0`, tag `v0.2.0`, push; CI drafts the release; download its artifacts into an empty dir, `shasum -c`, `ditto -x -k` the zip there so `Lumina.app` sits beside the zip and DMG, then `LUMINA_REQUIRE_DISTRIBUTION=1 scripts/verify_release.sh <that dir>`; publish the draft.

**Cannot be verified by an agent:** every step above that needs the certificate, Apple's notary service, the GitHub repo, Accessibility permission or a second Mac. Size: M (calendar time dominated by Apple account setup).

---

## Open questions for the owner

1. **Where does the repository live, and is it public?** There is no git remote. Release downloads, the README install link and SUPPORT.md's issue link all depend on it.
2. **Version of the first release:** ship the current `0.2.0` (never tagged, Decision: default) or bump to `0.2.1`?
3. **Notary credentials in CI:** Apple ID + app-specific password (already wired, personal account) or an App Store Connect API key (team-friendly, revocable; small script change)?
4. **Protect the signing secrets with a GitHub `release` environment** that needs your approval per run? Recommended if anyone else gets write access.
5. **Bundle identifier** `com.lumina.photos`: keep it? Changing it after release resets users' folder-access grants and preferences tied to the id; decide before v0.2.0.
6. **Oldest macOS you can test on.** If not 12, either test 12 or raise `LSMinimumSystemVersion` and the README claim to what was tested.
7. **Folder-access prompt text:** add `NSDesktopFolderUsageDescription`/`NSDocumentsFolderUsageDescription`/`NSDownloadsFolderUsageDescription`/`NSRemovableVolumesUsageDescription` strings so the first-run prompts say why? (Prompts appear without them, with generic text.)

## Out of scope

- In-app update check or auto-update (P2 0.7+, ROADMAP.md:542; 1.0 milestone).
- Windows/Linux artifacts (RELEASING.md acceptance criteria not met).
- Bit-for-bit reproducible builds, signed checksums (GPG/minisign), SBOMs.
- Running the OS-level e2e in GitHub CI (needs a TCC grant on the runner; stays a local, recorded release step).
- Resolving `curl`/`sips`/`open` by absolute path instead of `PATH` (hardening, not a notarization issue; separate fix if wanted).
- Mac App Store / sandboxing.
- Accessibility roles (AGENTS.md §11, separate milestone).
- `live_ui_automation.sh:280` printing `PASS` when interaction tests are skipped (conflicts with AGENTS.md §2 "never report a skipped test as a pass"). This plan sidesteps it by always setting `LUMINA_REQUIRE_UI_PERMISSIONS=1`; fixing the wording is a separate `fix:` commit.
- Notarizing with `allow-unsigned-executable-memory` or other extra entitlements for yt-dlp: not needed for the measured paths; add only with evidence from a failing real download.

## Review notes (2026-11-05)

Changed:
- Corrected citations: `package_release.sh:30-51`, `RELEASING.md:75` (was :79), `RELEASING.md:69-77` (was :73-81), `README.md:212`; added `LICENSE:3`.
- `check_macho_signatures`: a literal `flags=0x10000(runtime)` match would fail every ad-hoc + runtime build (`flags=0x10002(adhoc,runtime)`, measured). Changed to a regex; now checks every file in `Contents/MacOS`.
- `check_min_os`: yt-dlp's x86_64 slice has only `LC_VERSION_MIN_MACOSX` (10.13), which `vtool -show-build` does not print; the check now reads both load commands.
- Measured that `disable-library-validation` is required and sufficient for yt-dlp `--version` and `--list-impersonate-targets` under ad-hoc + runtime (arm64); added the second command to `check_helpers_start`; `check_entitlements` now compares against the entitlements file.
- Found `notarize_release.sh` leaving `Lumina-notarize.zip` in the upload dir (would be published unchecksummed); Task 2 uses a temp dir, `check_checksums` rejects unlisted archives.
- release.yml: release step tag-only (dispatch would create a release from a branch); HAS_* cover every required secret; identity stays `-` without a certificate.
- Moved packaging wiring of `verify_release.sh` from Task 1 to Task 2 (Task 1 alone would break `make release`).
- Added: notarytool JSON parsing with `plutil` and exit-code capture, `--timeout`; no-credential guard regression; DMG mount check; extracted-app staple check; Info.plist copyright edit (Task 6, resolved former open question 6 from `LICENSE`); "CI validated" step in Task 7; Rosetta fallback; RELEASING.md `--password`-on-command-line fix.

Could not verify: x86_64 slices (no Rosetta here); a real yt-dlp download under hardened runtime; anything needing Developer ID, the notary service or a GitHub remote; whether `macos-15` is still offered at release time; that notarytool's exit code is non-zero on `Invalid` (the plan handles both).
