# AGENTS.md - Engineering and Review Guide

> Audience: AI coding agents and human contributors working in this repository.
> This file is normative. If a local habit conflicts with it, follow this file.
> Update it when the team agrees on a new standard.

## 1. Project overview

Lumina is a local-first macOS photo and video manager:

- `lumina-core` is UI-free: SQLite catalog, scanner, decoder, search, smart
  albums, geocoding, caching, XMP sidecars, filesystem watcher.
- `lumina-ui-gpui` is the GPUI desktop app (GPUI Kit 0.9, which pins
  `gpui-pre` 0.3.11; imported as `gpui`): state, views, player bridge,
  icons, theme, and UI automation tests.
- Rust stable (1.95 or newer), edition 2021, workspace resolver 2.
- macOS is the only supported platform. A Linux port is in development
  (GStreamer player, system ffmpeg previews, internal `.deb`); it is not
  supported or released. See `RELEASING.md` and `docs/linux.md`.
- Version is synchronized across `crates/*/Cargo.toml`, `resources/Info.plist`,
  and the `Makefile`; release builds fail on mismatch.

Core promises: local-first with no background network or telemetry (the
network is used only for a URL the user asks to import, see §5.4); user data
is never silently lost; the UI never blocks on IO; every feature is testable
and reviewable.

## 2. Non-negotiable invariants

- Never commit `target/`, `screenshots/`, local config, credentials,
  `.DS_Store`, or user libraries.
- Tests must not write real user config or catalog data. Use temp directories
  and test-only early returns such as `save_config` under `#[cfg(test)]`.
- Never swallow a catalog/config write error. Return it, surface it in the error
  banner, and revert optimistic UI state. Log background failures with `log`.
- Never weaken a release performance gate to make a test pass. Investigate the
  query plan or algorithm instead.
- Never report a skipped test as a pass. State the reason and what was run.
- Do not add a dependency without a concrete need, license consideration, and
  CI compatibility. Registry access may be unavailable in constrained agents.

## 3. Setup and commands

Requirements: macOS 12 or newer, Xcode Command Line Tools (cc, AppKit,
AVFoundation), Rust stable.

```bash
make dev              # run debug app
make build            # cargo build --workspace
make test             # full test suite
make fmt              # rustfmt check
make lint             # clippy (all targets and features)
make bench            # opt-in, serialized performance gates
make ui-smoke         # launch real window, screenshot, shut down
make ui-e2e           # OS-level UI automation (needs Accessibility)
make bundle           # Lumina.app (universal when targets installed)
make release          # build + sign + zip/DMG + SHA256SUMS
make notarize         # release with notarization + stapling
```

Run commands from the repository root unless a task says otherwise.

## 4. Repository map

```text
crates/lumina-core/src/
  lib.rs          LuminaEngine API, events; public types in types.rs
  engine/         engine scans, thumbnails, duplicates, XMP sidecars
  catalog/        schema and migrations, media, tags, albums, smart and
                  system albums, search, trash, downloads, backups
  scanner.rs      filesystem scan and EXIF extraction (scanner/mp4.rs)
  decoder.rs      frame decode, native tool timeouts
  watcher.rs      incremental library watcher
  search/         structured query and FTS5 SQL
  smart_albums/   rule evaluation and SQL synthesis (rules_sql.rs)
  xmp/            sidecar render/parse/write
  net_import/     user-initiated URL video import
  cache.rs        memory/disk thumbnail cache
crates/lumina-ui-gpui/src/
  app/            state, actions, keymap, selection, undo, sort, command
                  palette, render
  views/          inspector, viewer, sidebar, modals, preferences
  i18n/           en-US catalog (en_us.rs), plurals, per-platform copy
  player/         AVFoundation (macOS) and GStreamer (Linux) players
  tests/          in-process UI automation
scripts/          live smoke/e2e, Linux packaging and release scripts
resources/        Info.plist, icns, icons, fonts, entitlements
docs/             accessibility, search, network import, Linux
```

## 5. Rust coding standards

### 5.1 Style and structure

- `cargo fmt --all` is authoritative. Do not hand-format against rustfmt.
- Clippy must pass with `-D warnings` for all targets and features. Prefer
  fixing code over `#[allow]`; narrow allows need a justification comment.
- Split modules that grow past roughly 500 lines or mix concerns. Do not add
  another large view or kitchen-sink state file.
- Public items have doc comments. Complex SQL, threading, and data-loss paths
  explain why, not obvious what.
- Remove dead code, unused imports, unused fields, and stale `allow(dead_code)`
  attributes unless a test-only helper genuinely needs one.
- Prefer explicit inputs and structured outputs. Do not grow tuples past two or
  three values; introduce a small struct.

### 5.2 Error handling

- Use `anyhow::Result` and `.context(...)` in engine and app code.
- Use `?` instead of `unwrap`/`expect` in production. `expect` is limited to
  startup invariants and tests, and must carry a useful message.
- Do not use `let _ =` for fallible catalog/config writes. Handle the error,
  revert optimistic state, and report it to the user.
- Background work (watcher, scan, cache cleanup) logs with `log::warn!` or
  `log::error!` and never panics the process.
- External native tools (sips, QuickLook) run through the timeout helper. Never
  block a decode worker forever on a subprocess.

### 5.3 State, concurrency, database

- SQLite access is serialized through the catalog connection. Do not hold the
  lock across UI awaits or long computations.
- Use transactions for multi-row mutations. Keep FTS rows and triggers in sync
  when adding metadata tables or columns.
- Schema changes require: an idempotent migration for existing databases, a
  `PRAGMA user_version` bump, and updates to every SELECT list, mapper, and
  query builder. Grep for `map_query_rows` and the media column list.
- Keep timeline buckets, metadata maps, and batch summaries fresh after edits.
  Clear or remap position-based selection when item order can change.
- Sort tie-breakers must follow the primary direction to match composite
  indexes. A mismatched tie-breaker silently turns a seek into a full sort.
  Check `EXPLAIN QUERY PLAN` when changing ORDER BY.
- Group related state in small structs rather than adding flat `LuminaApp`
  fields. Test code must not persist config.

### 5.4 Dependencies and platform

- Prefer the standard library and existing workspace crates. New crates need a
  concrete need, license review, and CI/platform compatibility.
- macOS-only code stays in clearly marked modules (`build.rs`, player bridge,
  release scripts). Headless core stays cross-platform where practical.
- Windows is a planned release target; Linux is in development, not supported.
  Do not add platform claims, CI release jobs, or "portable" abstractions until
  the porting and acceptance criteria in `RELEASING.md` are met. CI builds
  Lumina for macOS and Linux: it has a Linux build/test job; its
  `package-linux` job builds, checks and uploads an unsigned `.deb`, and its
  `bundle-macos` job uploads an ad-hoc signed `Lumina.app` zip, both as
  14-day workflow artifacts on pushes to main and on pull requests. The
  release workflow also builds and checks the unsigned `.deb` as a workflow
  artifact. These are internal, short-lived build artifacts: never attached to
  a GitHub release and never presented as a supported or distributable
  release (Linux not until `RELEASING.md`'s acceptance checks pass).
- Linux plays video through GStreamer (`player/linux.rs`). On Linux only,
  local previews (large photos, TIFF, video posters and scrub frames) may run
  the system ffmpeg (a `.deb` runtime dependency) as a separate process
  through the native tool timeout helper, restricted to the file protocol so
  it never uses the network. UI copy never names GStreamer or ffmpeg; their
  errors go to the log. macOS copy (Finder, the macOS Trash) stays as is;
  Linux wording lives beside it, cfg-selected, in `i18n::platform`.
- No telemetry, analytics, crash uploaders, or network calls without explicit
  product approval and documentation.
- Do not add Google Photos, iCloud Photos, Dropbox, OneDrive or Google Drive
  account integrations, remote library browsing, or two-way sync. The only
  approved future network feature that handles user media is optional backup to
  user-owned S3-compatible storage: credentials only in the OS credential store,
  client-side encryption, no secrets in config/database/logs, and no automatic
  deletion of local or remote data. Update checks remain explicit opt-in and
  must not upload user data.
- Approved exception: user-initiated URL video import (`net_import`, File >
  Import Video from URL). It contacts the site in the URL the user pasted and
  the servers that site sends the video from (CDN and API hosts), sends no
  user data, and runs nothing in the background. It runs system curl and the
  yt-dlp and ffmpeg bundled in the app, pinned by version and SHA-256 in
  `scripts/fetch_yt_dlp.sh` and `scripts/fetch_ffmpeg.sh` (an LGPL-only,
  copy-only ffmpeg built from source with network protocols disabled).
  Without the bundled copies (a local build) it falls back to ones the user
  installed (Homebrew, `PATH`). Lumina never downloads an executable at run
  time. The UI never
  names these tools: messages say what happened and what the user can do.
  See `docs/network-import.md`.

## 6. Testing strategy

### 6.1 Layers

1. Core unit tests live beside code in `lumina-core/src/**`.
2. Core integration tests live in `crates/lumina-core/tests/` (`core_tests.rs`
   plus import, watcher, URL import, format and benchmark tests).
3. UI automation tests live in `crates/lumina-ui-gpui/src/tests/` and drive the
   real GPUI view tree with synthetic input. They run headlessly in CI.
4. Live smoke/e2e scripts launch the real process; see section 8.

### 6.2 Test rules

- Every behavior change gets a test at the lowest layer that can prove it.
- Every bug fix gets a regression test that fails before the fix and passes after.
- Isolate test data: temp directories, unique names, cleanup on success and
  failure. Never depend on the developer home directory or an existing library.
- Tests are deterministic. Avoid sleeps; use event/render helpers. Timing
  assertions use generous debug budgets and strict release gates.
- Assert observable behavior (state, database rows, files, screenshots), not
  implementation trivia. A test that only checks no panic is not enough.
- Use assertion messages that state the violated rule.

### 6.3 GPUI-specific rules

- In modules that glob-import `gpui::*`, plain `#[test]` can resolve to the GPUI
  proc macro and crash rustc. Use `#[core::prelude::v1::test]` there, or import
  the standard test macro explicitly.
- Add stable `debug_selector` names to interactive controls. They are no-ops
  outside test/test-support builds and are part of the automation contract.
- Drive clicks at rendered bounds via `VisualTestContext`, not hard-coded
  coordinates, unless the test specifically targets a backdrop or coordinate.
- Refresh and draw the window to populate layout, then run until parked before
  asserting bounds.
- Overlays need an occluding card plus a dismiss backdrop. Otherwise synthetic
  clicks can hit the backdrop and close the overlay before the card responds.
- Focus the app before keyboard tests. Modal key handling must absorb background
  shortcuts so a dialog cannot mutate or delete media.

### 6.4 Commands and permissions

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
make bench
```

- The full suite must pass before a commit/PR.
- Benchmarks are opt-in and serialized for stable measurements. Do not run them
  alongside other heavy jobs.
- Live interaction tests need macOS WindowServer and Accessibility/PostEvent
  permission for the harness. Without it `make ui-e2e` reports SKIP and exits
  successfully. Use `LUMINA_REQUIRE_UI_PERMISSIONS=1` when permission is
  mandatory. Never fake a passing interaction test.

## 7. UI automation

- Prefer in-process automation for regression coverage; it is fast and runs in CI.
- Add a `debug_selector` for every control a test needs (toolbar, search, cards,
  modals, palette rows, preferences tabs). Reuse stable names; changing one is a
  test-contract change.
- Test the real rendered tree: synthetic click at center bounds, typed text,
  keyboard sequences, resizes, double and right clicks, backdrop dismissal.
- Cover both the input path and the persisted result (config, database, sidecar,
  files) when the action is data-changing.
- Keep dedicated tests for keyboard-only flows (command palette, shortcut sheet,
  search focus, undo/redo). Mouse-only coverage is insufficient.
- Do not commit screenshots or generated automation artifacts. They live under
  `target/` or `screenshots/`, both ignored.
- When a live e2e check is skipped, report the permission or headless reason.

## 8. Smoke tests

Run smoke checks for UI, packaging, dependency, or launch-path changes:

```bash
make ui-smoke                     # real window, screenshot, clean shutdown
make ui-e2e                       # live interaction; skips without permission
LUMINA_REQUIRE_UI_PERMISSIONS=1 make ui-e2e   # permission is expected
```

Artifacts land in `target/ui-smoke/` and `target/ui-e2e/`; screenshot capture
requires Screen Recording. A smoke check is not a substitute for unit tests,
and a skipped check is not a pass.

Release checks:

```bash
LUMINA_SIGN_IDENTITY="Developer ID Application: ..." make release
LUMINA_NOTARY_PROFILE=LuminaNotary make notarize
codesign --verify --deep --strict --verbose=2 target/release/bundle/osx/Lumina.app
hdiutil verify target/release/bundle/osx/Lumina-*.dmg
cd target/release/bundle/osx && shasum -a 256 -c SHA256SUMS.txt
```

Unsigned/ad-hoc artifacts are for local testing only and must not be presented
as a distributable release. Signing requires certificates and secrets described
in `RELEASING.md`.

## 9. Independent agent reviews

Implementation and review must be separate steps. An author agent must not be
the only reviewer of its own work.

Reviewer responsibilities:

- Start from the requirement and the diff, not from the author summary.
- Read the full changed files, not only changed hunks. Check surrounding state,
  error paths, and data migration impact.
- Run the gates: fmt, clippy, tests, and benchmark/UI smoke when relevant.
- Verify a regression test would fail without the fix when practical (revert the
  fix locally or reason explicitly why that is not feasible).
- Check YAGNI, data safety, performance index usage, accessibility, and whether
  the change is the smallest correct implementation.
- Report blockers separately from nits, with file and line references, and state
  what evidence is missing.
- Do not edit while reviewing. The author fixes findings, then the reviewer
  re-checks the changed files and gates.

When multiple agents work in parallel:

- Give each agent a disjoint write scope and record it on the shared task.
- Do not let two agents edit the same file or run conflicting formatters.
- Use task dependencies for ordering and claim the latest task revision.
- The Lead must review the final diff, confirm a clean working tree, and run the
  full gate before declaring completion.

Review checklist:

- [ ] Requirement met; edge cases and error paths handled.
- [ ] No swallowed writes; data loss or corruption impossible in normal paths.
- [ ] Tests exercise the failure mode fixed and run deterministically.
- [ ] No unrelated changes, dead code, or speculative features.
- [ ] Query plans/indexes considered; no blocking work on the UI thread.
- [ ] Keyboard path and accessibility impact considered.
- [ ] Docs/help or release notes updated when user-visible.
- [ ] No secrets, generated artifacts, or local config added.

## 10. Performance testing

- Performance work is measured, not guessed. Capture a before and after number.
- Use `make bench` (serialized, opt-in) for throughput and latency gates. Do not
  run it concurrently with compilation or other heavy work.
- Debug budgets are intentionally looser than release gates. Release gates stay
  strict (timeline seek under 1 ms, structured query under 5 ms, smart album
  under 8 ms). Do not relax them to avoid an investigation.
- Investigate an ORDER BY change with `EXPLAIN QUERY PLAN`; keep composite index
  and tie-breaker directions aligned.
- Avoid per-frame SQLite queries. Cache maps and summaries, virtualize lists, and
  measure startup, memory, and thumbnail throughput when touching those paths.
- The CI perf job runs the benchmark suite; a performance regression is a
  blocker, not a flaky test.

## 11. Accessibility

- Every user-visible action must have a keyboard path. Add global actions to the
  command palette (`Cmd+K`) and, when appropriate, the menu bar and shortcut sheet.
- Keyboard flows must be tested: focus, Escape handling, arrow/Enter behavior,
  and modal key absorption.
- Do not add mouse-only controls. Icon-only controls get tooltips and labels.
- Keep focus and selection states visible, and preserve scroll-into-view for
  keyboard navigation.
- GPUI reports elements to VoiceOver through AccessKit only when they have an
  `id` and a `.role(...)` (plus `aria_label` and friends). The text fields
  and many controls (buttons, menus, lists, toolbars, alerts) set roles, but
  coverage is partial and the tree has not been checked with VoiceOver.
  Give new controls a role and label. Do not claim screen-reader support that
  has not been checked with VoiceOver; native menus and the command palette
  are the practical paths. See `docs/accessibility.md`.
- Localization: user-facing command labels go through `src/i18n/` (`i18n::t`,
  `i18n::plural`). New strings should use catalog keys where practical; en-US
  is the only shipped locale today.

## 12. YAGNI and design review

Apply YAGNI to implementation and review:

- Is this required by the current task or a documented near-term need?
- What is the smallest change that satisfies it? Could existing code or the
  standard library handle it?
- Does this add a dependency, trait, generic, config flag, or abstraction that
  has only one caller and no proven need?
- Is the feature complete enough to use, test, and explain? Partial speculative
  scaffolding is usually worse than no scaffolding.
- Delete superseded code, dead branches, and unused fields instead of leaving
  them for a hypothetical future.

YAGNI does not excuse cutting reliability, error handling, tests, accessibility,
data safety, or required documentation. Those are part of the feature.

## 13. Git, commits, and releases

- Commit one logical change. Use `feat:`, `fix:`, `test:`, `docs:`, `ci:`, or
  `chore:` prefixes, with a body explaining behavior and verification.
- Do not commit generated files, screenshots, build artifacts, or secrets.
- Before committing: `cargo fmt --all -- --check`, clippy, tests, and the relevant
  smoke/bench checks. Report anything skipped.
- Keep versions in sync across Cargo, Info.plist, and the Makefile. Release
  scripts validate this and fail on mismatch.
- Release workflow is tag-driven (`v*`) and signed/notarized only when secrets are
  configured. See `RELEASING.md`. Unsigned artifacts are drafts, not releases.

## 14. Definition of done

A change is done only when all applicable items are true:

- [ ] Behavior implemented with the smallest correct change.
- [ ] fmt, clippy, and full tests pass; new tests fail without the change.
- [ ] Error paths surfaced; database writes and migrations handled.
- [ ] Performance considered and benchmarked when hot paths or SQL changed.
- [ ] Keyboard and accessibility impact handled; docs updated.
- [ ] Independent review completed; blockers fixed and re-checked.
- [ ] UI smoke/e2e run when the change affects startup, layout, or interaction.
- [ ] Working tree clean; no secrets or generated artifacts; commit message
      states what was verified and any known limitations.

## 15. Known limitations and honest reporting

- Semantic/AI search is not implemented in this tree; do not describe it as
  complete.
- OS-level UI interaction tests require Accessibility permission; skip explicitly.
- AccessKit roles cover the text fields and many controls but not all, and
  the element tree has not been checked with VoiceOver; native menus and the
  palette are the current accessible paths.
- Signing/notarization requires Apple credentials; CI release artifacts are
  drafts until verified and published.
- DMG creation may be blocked in sandboxed sessions; use `LUMINA_SKIP_DMG=1`
  only for local verification, not as a release substitute.
- Windows is a planned release target; Linux is in development (GStreamer
  player, ffmpeg previews, no HEIC or RAW previews); its CI job and internal
  `.deb` are not a release. Keep releases macOS-only until the acceptance
  criteria in `RELEASING.md` are met.
- S3-compatible backup is planned for the mid-roadmap (0.5) and is not
  implemented. Cloud photo-library accounts (Google/iCloud) and two-way sync are
  out of scope; do not describe them as supported.
- Photos are converted to sRGB on macOS: wide gamut is clipped, not shown on
  wide-colour displays. The QuickLook fallback for RAW is unchecked, and
  non-macOS decodes are not converted.
- RAW is the embedded or system rendering, not a develop pipeline, and has not
  been checked on real files (`raw_fixture_matrix` needs
  `LUMINA_RAW_FIXTURES`); do not describe RAW size or colour as verified.
- Lumina is MIT licensed. Do not relicense the project or remove the LICENSE
  file without maintainer approval, and keep bundled third-party notices accurate.

When blocked or uncertain, state the concrete condition and the evidence, not a
guess. Remaining work is better than a false completion claim.

## 16. Maintaining this guide

Update this file when commands, conventions, CI jobs, or review expectations
change. Keep it actionable: every rule should help an agent produce a smaller,
safer, better-tested change.
