# Lumina Product Roadmap

This is the forward-looking product roadmap. The historical implementation plan
for the v0.2 architecture lives in [docs/roadmap_v2.md](docs/roadmap_v2.md).

Guiding principles:

- **Local-first and private** — no accounts, telemetry, or cloud dependency.
- **Data safety first** — catalog writes, metadata, sidecars, and files must be
  recoverable. No silent failures.
- **Keyboard-first and accessible** — every action has a keyboard path; no
  mouse-only features.
- **YAGNI, shipped small** — build the smallest complete feature, test it, and
  only then extend it. No speculative subsystems.
- **Measure before optimizing** — performance changes carry before/after data.
- **Honest support** — do not claim a platform, format, or accessibility level
  that is not tested and released.

## Privacy and credentials model

Local-first is a product constraint, not a temporary state:

- No Lumina account. No Google Photos, iCloud Photos, Dropbox, OneDrive or
  Google Drive account integration; no remote library browsing and no two-way
  sync.
- No telemetry, analytics, crash uploads or background network calls by default.
- Network use is limited to explicit user actions: **optional backup to
  user-owned S3-compatible storage** (post-1.0, see section 14) and opt-in update
  checks. Neither creates a Lumina account or a cloud photo service.
- Backup credentials belong in the OS credential store (macOS Keychain, Windows
  Credential Manager, Linux Secret Service), never in config.json, SQLite, logs
  or diagnostics.
- Backup manifests, catalog snapshots and XMP sidecars are client-side encrypted
  by default with a user-held key; Lumina does not escrow keys. Losing the key
  means losing access to that encrypted backup.
- Local or remote data is never auto-deleted. Restores and deletions require
  explicit confirmation.

## Current baseline (v0.2.0, macOS)

**Shipped:** macOS app; SQLite catalog with FTS5; EXIF/GPS extraction with
offline geocoding; structured search; system and user smart albums; virtualized
grid; zoom/pan viewer; AVFoundation video playback; ratings, colors, tags,
captions, favorites; title/date/location editing; undo/redo; XMP sidecars with
export/import; persisted window/UI preferences; tag manager with rename, merge,
bulk delete and sorting; multi-select with batch favorite/tags/trash/export/rating/color;
sorting across all views; native menu bar; preferences; command palette;
crash/logging safeguards; single-instance lock; DB backups and cache eviction;
release scripts and CI perf/coverage/audit jobs.

**Verification baseline:** 100+ workspace tests including in-process UI
automation; fmt/clippy clean; serialized benchmark gates; live window smoke and
OS-level e2e base checks. OS-level interaction tests still skip without
Accessibility/PostEvent permission. The release pipeline has not yet been
exercised with real signing/notarization credentials.

**Known gaps at this baseline:** first-run onboarding and real import are
minimal; no copy-into-library workflow or drag-and-drop import; tags are flat
and there is no first-class Untagged view; albums are smart-only (no curated
static albums); batch editing covers ratings/colors/tags/trash but not
title/date/location/caption; video lacks resume, chapters, subtitle/audio track
selection, and scrub previews; no compare, slideshow, or map view; no light
theme; no on-device semantic search; Windows/Linux are planned but unsupported;
VoiceOver has no GPUI element tree; no in-app update mechanism.

## The Grade A quality bar

A release is Grade A when a new user can do the following without confusion,
data loss, or a mouse:

1. Install a signed app, open it, and understand the first action immediately.
2. Import or open a library, see progress, cancel safely, and recover from errors.
3. Find media by browsing, search, dates, tags, ratings, colors, and smart rules.
4. Organize media with tags, albums, batch changes, and undo everywhere.
5. View photos and videos fluently, including compare, slideshow, map, and video
   basics (resume, scrubbing, audio, PiP).
6. Export originals and metadata without lock-in; import that metadata back.
7. Use every core action from the keyboard, menu, or command palette.
8. Get help, diagnostics, and backup/restore without reading source code.
9. Experience no crashes, hangs, or silent write failures in the test matrix.
10. Trust the build: signed, notarized, reproducible checksums, and documented
    platform support.

## Milestones

| Milestone | Theme | Primary outcomes |
| --- | --- | --- |
| **0.2.x** | Release hardening | Signed/notarized macOS release; CI validated; e2e permission run; release notes and support docs. |
| **0.3** | First-run and import | Onboarding, real import/copy workflow, drag-and-drop, recent libraries, progress/error recovery. |
| **0.4** | Organize | Tags hierarchy and Untagged, curated albums, smart-album polish, full batch edit and undo. |
| **0.5** | Media experience | Video improvements, compare/slideshow/map, share and export presets. |
| **0.6** | Polish and access | Theme/accent, toasts/empty/loading states, help, accessibility, localization. |
| **0.7** | Cross-platform | Windows and Linux ports, native packages, per-platform CI and signing. |
| **0.8** | Semantic search | On-device embeddings and hybrid search after the core experience is solid. |
| **1.0** | Grade A | Hardening, docs, update mechanism, support policy, full platform test matrix. |
| **1.1** | Private backup | Optional client-side-encrypted backup to user-owned S3-compatible storage; OS credential storage; verify/restore; no sync or browsing. |

Milestones are sequential for a reason: import and organization create the
library that later features such as duplicate detection and semantic search
need to be useful. Dates are intentionally omitted until scope is estimated.

## 1. Onboarding and first-run

**Current:** a welcome screen with one primary action (Choose Photos Folder) and
one shortcut to Pictures. No guided setup, sample library, import options,
recent libraries, or recovery guidance.

**Target:**

- First-run flow with three clear paths: open an existing library, import/copy
  media into a new library, or explore a small built-in sample library.
- Explain the local-first model and where catalog/cache/config live.
- Folder validation, permission explanation, and a safe fallback when a folder
  is unreadable or empty.
- Import options: copy into the library or index in place; sidecar handling;
  duplicate policy preview.
- Real progress with item counts, current file, cancel, and resume-on-next-launch
  where safe.
- Recent libraries list, remove-from-recents, and move/relink handling.

**Acceptance:** a new user reaches a populated library in under 30 seconds on a
normal folder; cancellation leaves no partial corruption; every failure has an
actionable message and a retry path; the flow is keyboard-operable and covered
by UI automation.

## 2. Import, library lifecycle, and file management

**Current:** opening a folder indexes it in place. There is no copy-into-library
import, drag-and-drop ingestion, multi-root library, missing-file relink, or
in-app backup/restore.

**Target:**

- Import from folder picker and drag-and-drop, with copy/index-in-place modes.
- Duplicate detection during import (content hash plus size, with a review step).
- Import queue with progress, failures, retry, and safe cancellation.
- Multiple watched library roots and removable roots.
- Missing-file detection, relink, and a clear repair workflow.
- Trash policy controls: move to macOS Trash vs catalog trash vs permanent
  delete, with confirmation and undo where feasible.
- Backup/restore UI: catalog snapshot, XMP sidecars, and a library health report.

**Acceptance:** import can be cancelled and resumed without duplicates; every
file operation is reversible or explicitly confirmed; no media is deleted
without visible confirmation; missing-file repairs have tests.

## 3. Tags, untagged media, and cleanup

**Current:** tag manager supports search, rename, merge, delete, bulk delete,
and sort. Tags are flat; tagging is applied from the inspector/batch dock; an
Untagged smart-album template exists but there is no first-class Untagged view
or cleanup workflow.

**Target:**

- First-class **Untagged** system album in the sidebar with live count and a
  "start tagging" empty state.
- Optional first-class **Unrated**, **No color label**, and **No camera** quick
  smart collections.
- Tag hierarchy/collections (parent/child) or an approved flat-tag alternative,
  decided by a short design spike before implementation.
- Autocomplete from library tags, recently used tags, and keyboard-first
  assignment without opening the inspector.
- Drag tags onto a selection, remove tags from a selection, and bulk rename with
  a preview of affected items.
- tag colors/pins only if they improve scanning; avoid decorative complexity.
- Undo/redo for tag rename, merge, delete, and bulk assignment.
- Tag aliases/synonyms and a cleanup report for near-duplicate tags.

**Acceptance:** a user can go from Untagged to organized without touching the
mouse; tag operations are undoable; merge/rename preserves all item links;
tag counts and search stay correct after every operation.

## 4. Smart albums, curated albums, and organization

**Current:** system smart albums plus user smart albums with create/edit/delete
and rule builder. No curated/static albums, album previews, pinning, nesting,
or rule import/export.

**Target:**

- **Curated albums** (static selections) separate from smart albums, with drag
  and keyboard add/remove.
- Album cover/preview thumbnails and live item counts.
- Pin/favorite/reorder albums; nested folders/collections.
- Album descriptions, custom icons, and optional colors.
- Duplicate a smart album and import/export rule JSON.
- Richer smart rules: relative dates, camera/lens sets, rating/color ranges,
  location radius, media duration, and "similar to this item" (later).
- Smart-album empty state and explanation of why an item matched.

**Acceptance:** albums are discoverable, editable, and undoable; rules are
portable; counts update after watcher changes; no orphaned albums after item
deletion.

## 5. Multi-select and batch editing

**Current:** range/cmd selection, select-all, batch favorite/tags/trash/export,
and batch rating/color in the inspector. No rubber-band selection, mixed-value
editing, batch title/date/location/caption, or grouped undo.

**Target:**

- Rubber-band drag selection and keyboard extend/shrink.
- Mixed-value inspector for rating, color, tags, title, caption, date, and
  location with explicit "apply to N items" confirmation.
- Batch edit title/date/location/caption and copy/paste metadata.
- One undo entry for a batch action, with a readable label.
- "Select similar" (same date, camera, location, tag) and select-all-in-view.
- Drag a selection to export, trash, an album, or a tag.
- Optional batch rename/file move with a dry-run preview (higher risk; later).

**Acceptance:** no accidental partial edits; mixed values are shown honestly;
undo restores every touched item in one step; large selections stay responsive.

## 6. Video player

**Current:** AVFoundation playback with play/pause, scrubbing, frame stepping,
speed menu, mute/volume, loop, PiP, fullscreen, and hover seek. No resume,
scrub thumbnails, chapters, subtitle/audio track selection, or loop range.

**Target:**

- Resume playback position per video, persisted across sessions.
- Thumbnail previews while scrubbing and a buffered/progress indicator.
- Chapter/bookmark support from metadata and user bookmarks.
- Audio track and subtitle track selection; remember last choices.
- Loop range (in/out points) in addition to full-loop.
- Persistent playback speed/volume with sensible defaults.
- Keyboard frame stepping with visible frame/time feedback; precision scrub mode.
- PiP controls and keyboard focus polish; recover gracefully from unsupported
  codecs or missing audio.
- Later: simple trim/export clip, if it fits the non-editor scope.

**Acceptance:** playback survives window/PiP transitions; resume is accurate;
unsupported formats fail with a clear message; video shortcuts are documented
and tested.

## 7. Browsing, search, and viewer

**Current:** structured query grammar, FTS5 search, timeline buckets, sorting,
grid density, zoom/pan, filmstrip, keyboard navigation. No saved searches,
search history, quick rating/color filters, compare, slideshow, or map.

**Target:**

- Saved searches and recent search history, with a displayable query chip.
- Search suggestions/autocomplete for cameras, lenses, cities, tags, and
  structured operators.
- Quick filter chips for rating and color, not just favorite/video/raw/place.
- Compare view (2-up and 4-up), synchronized zoom/pan.
- Slideshow with interval, loop, and optional music-free presentation mode.
- Map view using the existing offline geocoder and GPS metadata.
- Duplicate detection and burst stacks with a review UI.
- Quick Look/Finder integration and drag-out to Finder.

**Acceptance:** search is discoverable without memorizing the grammar; compare
and slideshow are keyboard-operable; map/duplicate views have empty and error
states; no feature blocks the UI thread.

## 8. UX polish and system integration

**Current:** dark graphite theme, status bar feedback, error banner, preferences
(General/Storage/About), native menus, command palette, tooltips, shortcut
sheet, persisted window/UI state. No light theme, accent customization, toasts,
skeletons, shortcut customization, update mechanism, or diagnostics bundle.

**Target:**

- Light theme and an accent-color choice; high-contrast option.
- Consistent toasts for success/failure and undo affordances; do not rely only
  on the status bar.
- Skeleton/loading states for thumbnails, import, search, and large albums.
- Better empty states with the next action and a keyboard hint.
- Preferences expansion: appearance, thumbnail cache limits, import behavior,
  video defaults, backup/restore, diagnostics.
- User-customizable keyboard shortcuts with conflict detection.
- Recent libraries, reopen last window/tab, and multi-window support if the
  product needs it.
- In-app update check (explicitly opt-in; local-first constraint) and release
  notes.
- "Export diagnostics" bundle: logs, versions, platform, cache/DB sizes, no
  private media data by default.

**Acceptance:** every interactive control has hover/active/focus/disabled states;
errors are actionable; no dead ends; polish changes are covered by visual/UI
automation where practical.

## 9. Accessibility and localization

**Current:** keyboard-first navigation, command palette, native menu bar,
tooltips, shortcut sheet. GPUI 0.2.2 exposes no NSAccessibility tree, so
VoiceOver sees only the window. i18n catalog exists but ships en-US only.

**Target:**

- An accessibility bridge (upstream GPUI contribution or a small platform layer)
  exposing roles, names, values, and focus order to VoiceOver.
- Complete focus order and visible focus rings across toolbar, grid, inspector,
  modals, preferences, and video controls.
- Reduced-motion preference and respect for system settings.
- Large-text/scaling checks for core layouts.
- Full keyboard walkthrough documented and tested.
- Locale catalogs for at least one non-English locale; RTL layout audit.
- Accessibility acceptance checklist in the release process.

**Acceptance:** a VoiceOver user can import/open a library, search, select, tag,
view, and export without a mouse; no keyboard trap; documented limitations.

## 10. Data safety, portability, and diagnostics

**Current:** SQLite migrations and backups, sidecars for title/rating/tags/caption,
undo, single-instance lock, logging, cache eviction. Date/location are not in
sidecars; no backup/restore UI; global tag operations are outside undo.

**Target:**

- XMP date/GPS write and import; auto-import/export on scan as a preference.
- Catalog backup/restore UI and an integrity report.
- Library health check: missing files, unreadable sidecars, FTS consistency,
  stale cache.
- Undo coverage for global tag operations and batch edits.
- Crash-safe recovery prompts after an interrupted import or migration.
- Diagnostics export and a clear privacy policy for logs.

**Acceptance:** a user can move a library to another machine with media +
sidecars + catalog backup and recover organization; no metadata is trapped in
a single database.

## 11. Performance, reliability, and scale

**Current:** 250k synthetic insert/search/smart-album gates; cache budgets;
background scanning/decoding; UI automation; CI perf/coverage/audit jobs.

**Target:**

- Startup budget and time-to-first-thumbnail measurement on a real library.
- Import throughput and memory ceiling under a 250k library.
- Background job cancellation and progress guarantees under watcher activity.
- Failure-injection tests: locked DB, full disk, unreadable file, corrupt sidecar,
  denied permission.
- Long-running soak test with watcher events during scan.
- Reproducible release builds and dependency audit review.

**Acceptance:** no UI-thread stalls over a defined budget; no panics in the test
matrix; every failure has a recovery path; perf gates stay green in release
mode (seek <1 ms, query <5 ms, smart album <8 ms).

## 12. Platform and release

**Current:** macOS 12+ only; universal build, signing, notarization, DMG/zip and
tag-driven release workflow are implemented but not yet exercised with real
credentials. Windows/Linux are planned in [RELEASING.md](RELEASING.md#windows-and-linux-planned).

**Target:**

- Cut signed/notarized 0.2.x release; verify on a clean Mac; publish notes and
  checksums.
- Add auto-update with signed feeds and rollback-safe behavior.
- Windows/Linux port per the documented workstreams: compile/test matrix,
  platform media backends, packaging/signing, and acceptance criteria.
- CI matrix across macOS/Windows/Linux; platform smoke tests.
- Support policy and known-limitations page per release.

**Acceptance:** users can install and update a signed build; artifacts verify;
cross-platform claims exist only after the documented acceptance criteria pass.

## 13. Semantic search (Phase 10)

**Current:** not implemented. Structured and FTS search only.

**Target design spike first:**

- On-device embedding model (size, license, quality, languages).
- Vector storage and incremental indexing in SQLite; no network.
- Hybrid ranking with structured/FTS filters; explainable results.
- Background embedding worker with progress, cancellation, and cache budget.
- UX: a clear "search by meaning" entry point, result provenance, and the
  ability to disable/delete embeddings.

**Acceptance:** a design document with model/size/privacy tradeoffs is approved
before implementation; offline tests prove indexing, ranking, and memory limits.

## 14. Optional private backup to S3-compatible storage (post-1.0)

This is the only planned network feature that handles user media. It is
deliberately narrower than cloud support: no accounts, no remote browsing, no
two-way sync, no sharing.

**Scope:**

- Manual or optional scheduled backup of originals, XMP sidecars and a catalog
  snapshot to a bucket the user owns and controls.
- User supplies endpoint, region, bucket, prefix and credentials; support AWS
  S3, Cloudflare R2, Backblaze B2 (S3 API), Wasabi, MinIO and other
  S3-compatible stores. Path-style and virtual-host addressing.
- Credentials live only in the OS credential store. Provide test connection,
  revoke and rotate flows; redact credentials from logs and error messages.
- Resumable multipart uploads, SHA-256 manifests/checksums, retry with backoff,
  bandwidth/concurrency limits and an offline queue.
- Client-side encryption for manifests, catalog snapshots and sidecars with a
  user-held key; originals can optionally be encrypted too. No key escrow.
- Incremental backup, dry-run preview and a verify mode that checks remote
  checksums without downloading everything.
- Restore into an empty library first; never overwrite or delete local data
  without confirmation. Never auto-delete remote objects.
- Settings > Backup with status, last successful run, failures, pause/cancel and
  clear quota/auth errors.
- Tests: mocked S3 plus MinIO/contract tests, partial upload/failure injection,
  credential-redaction tests, restore round-trip and documented key-loss
  behavior.

**Explicitly out of scope for this feature:** Google Photos, iCloud Photos,
Dropbox, OneDrive and Google Drive accounts; remote library browsing; sync
conflicts; remote sharing; any server operated by Lumina.

**Acceptance:** a user can configure a bucket with least-privilege credentials,
back up and restore a library with verified checksums, disconnect or rotate
credentials, and see no secrets in config, database, logs or diagnostics.

## Backlog: candidate features and priority

| Priority | Item | Milestone | Notes |
| --- | --- | --- | --- |
| P0 | Signed/notarized release + CI validation | 0.2.x | Needs Apple credentials and a native CI run. |
| P0 | First-run onboarding + import/copy | 0.3 | Core new-user and data-ingestion path. |
| P0 | First-class Untagged + tag cleanup | 0.4 | High everyday value. |
| P0 | Batch metadata edit + grouped undo | 0.4 | Title/date/location/caption and mixed values. |
| P1 | Curated albums | 0.4 | Needed alongside smart albums. |
| P1 | Smart-album previews, pinning, rule import/export | 0.4 | Organization polish. |
| P1 | Video resume, scrub previews, subtitles/audio tracks | 0.5 | Viewer parity. |
| P1 | Compare, slideshow, map | 0.5 | Browse and presentation. |
| P1 | Duplicate detection and burst stacks | 0.5 | Large-library cleanup. |
| P1 | Share sheet and export presets | 0.5 | Distribution workflows. |
| P1 | Light theme/accent, toasts, skeletons | 0.6 | UX consistency. |
| P1 | Accessibility bridge, focus order, reduced motion | 0.6 | Grade A requirement. |
| P1 | Locale catalog (one non-English locale) | 0.6 | i18n proof. |
| P1 | XMP date/GPS + backup/restore UI | 0.4-0.6 | Portability and safety. |
| P1 | Windows/Linux port | 0.7 | Large; follow RELEASING plan. |
| P2 | In-app updates and release notes | 0.6+ | Requires feed/signing infrastructure. |
| P2 | Diagnostics bundle and health check | 0.6 | Support quality. |
| P2 | Custom keyboard shortcuts | 0.6+ | Power-user polish. |
| P2 | Batch rename/file move | 0.5+ | Higher risk; dry-run first. |
| P2 | Quick Look/Finder extension | 0.5+ | Nice system integration. |
| P2 | Semantic search | 0.8 | Design spike first. |
| P2 | Optional client-side-encrypted S3-compatible backup | 1.1 | User-owned bucket, OS credential store, no sync/browsing. |

## Out of scope for the near term

- Google Photos, iCloud Photos, Dropbox, OneDrive and Google Drive account
  integration, remote library browsing and two-way sync. Optional S3-compatible
  backup (section 14) is separate and post-1.0.
- Cloud sharing/social services. The native macOS share sheet remains in scope.
- Generative AI editing or cloud inference.
- Telemetry or analytics by default.
- A full professional RAW develop/editor suite; basic non-destructive rotate/crop
  may be considered later, but Lumina is a manager, not Lightroom.
- Mobile apps and a plugin marketplace.

## Definition of done for roadmap items

- User-visible behavior is implemented and documented.
- Unit/integration/UI tests cover the happy path and failure modes.
- Undo and data-safety behavior are explicit.
- Keyboard and accessibility paths are covered.
- Performance is measured for hot paths and large libraries.
- Windows/Linux impact is considered (do not add more macOS-only assumptions
  without documenting the port cost).
- The change passes fmt, clippy, full tests, relevant smoke/bench checks, and
  independent review.

## Maintaining this roadmap

Update this file at each release: move completed items into the baseline,
remove or defer stale bets, and keep milestones small enough to finish. A
feature belongs here only if it has a clear user problem, a measurable outcome,
and an owner. Anything else waits in the backlog.
