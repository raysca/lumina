# Lumina Product Roadmap

This is the forward-looking product roadmap. The historical implementation plan
for the v0.2 architecture lives in [docs/roadmap_v2.md](docs/roadmap_v2.md).
Everyday interaction details live in [docs/ux_audit.md](docs/ux_audit.md).

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
  user-owned S3-compatible storage** (mid-roadmap, see section 15) and opt-in update
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
selection, and scrub previews; no map view (compare/survey and the
slideshow landed in plan 5 C1 and C2); no light
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

| Milestone | Theme | Band | Primary outcomes |
| --- | --- | --- | --- |
| **0.2.x** | Release hardening | Top | Signed/notarized macOS release; CI validated; e2e permission run; release notes and support docs. |
| **0.3** | Import, onboarding and everyday UX | **Top** | Import/copy, drag-and-drop, templates/verification, migration importers, recent libraries, plus UX-1: selection/open semantics, navigation continuity, feedback and motion foundation. |
| **0.4** | Metadata, keywords, quick edits and precision UX | **Top** | Hierarchical keywords, IPTC/XMP templates, batch metadata, quick edits, external editor handoff, plus UX-2: grid/viewer zoom, video drag-scrub/resume, search/tag polish. |
| **0.5** | Private S3-compatible backup | **Mid** | Optional client-side-encrypted backup to user-owned S3-compatible storage; OS credential storage; verify/restore; no sync or browsing. |
| **0.6** | Organize and media experience | **Mid** | Curated albums, smart-album polish, culling/pick-reject, compare/slideshow/map, video improvements, duplicates/stacks, share/export presets, RAW/HEIC/color validation. |
| **0.7** | Polish and access | **Mid** | Theme/accent, toasts/empty/loading states, help, accessibility, localization. |
| **0.8** | Cross-platform | Lower | Windows and Linux ports, native packages, per-platform CI and signing. |
| **1.0** | Grade A | Top | Hardening, docs, update mechanism, support policy, full platform test matrix. |
| **1.1** | Local intelligence | **Down** | People and pets clustering, OCR/object indexing, memories and auto-curation. |
| **1.2** | Semantic search | **Down** | On-device embeddings and hybrid search after the core experience is solid. |

Milestones are sequential for a reason: import and organization create the
library that later features such as duplicate detection and semantic search
need to be useful. Dates are intentionally omitted until scope is estimated.

## Priority order

**Top (0.2.x-0.4):**

1. Import/copy and onboarding, plus UX-1 feel and orientation. This is the first
   feature wave.
2. Metadata, keywords and quick edits, plus UX-2 precision browsing. This is the
   second feature wave.

**Mid (0.5-0.7):**

3. Private S3-compatible backup.
4. Organize/media workflow (albums, culling, video, map, duplicates).
5. UX polish, accessibility and localization.

**Lower / down the release roadmap:**

6. Cross-platform port.
7. People/pets, OCR/object indexing, memories and auto-curation.
8. Semantic search.

This order is intentional. Cloud photo libraries remain out of scope; the only
network feature that handles user media is the optional S3 backup in the middle
of the roadmap.


## 1. Onboarding and first-run

**Current:** a welcome screen with three paths, Open Existing Library… (↵),
Import into New Library… (⇧⌘I: choose or create a folder, it opens, and once
its first scan is done Lumina asks for the photos to copy in) and Explore a
Sample Library (⌥↵: 24 generated gradient photos dated over the last 90 days,
written off the UI thread to `Application Support/lumina/Sample Library` and
then opened as an ordinary library, so it lands in recents), a secondary Use
Pictures Folder link (⇧↵), copy that explains local-first storage (works on
photos where they are or copies them in, never changes originals, catalog in
Application Support), and a Recent Libraries list (up to 8, most recent
first; ↑/↓ and ↵ open one, ⌫ or × forgets one without touching its folder or
catalog; a missing folder is marked "Not available"). If the last library is
gone at launch, the welcome screen says which folder is missing. ⌘K has Open
Recent Library and Clear Recent Libraries. No guided setup or import options
yet; the sample photos are plain gradients (no EXIF, places or people).

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

**Current:** opening a folder indexes it in place. Files gone from disk are
kept as missing (badge and Missing view); moves inside the library reconnect
by inode, and "Locate…" relinks by hand. Import copies dropped or picked files
and folders into `Imports/` by date, skipping duplicates. There is no
index-in-place import of outside folders, multi-root library, or in-app
backup/restore.

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

## 3. Metadata, keywords, and quick edits (top priority)

**Current:** tags are flat; metadata editing covers title/date/location/caption
but not IPTC rights/creator fields; XMP is partial; there are no
non-destructive edits or external editor handoff. This is the second top
priority after import/copy.

**Metadata and keywords:**

- Hierarchical keywords/collections with aliases, autocomplete, controlled
  vocabulary and bulk apply (coordinated with the tag manager in section 4).
- IPTC/XMP templates: creator, copyright, rights usage, contact, keywords, date
  offset and GPS; apply to a selection, import/export templates, and write
  sidecars.
- Batch metadata copy/paste and reusable templates.

**Quick edits:**

- Non-destructive rotate, crop, straighten, auto-enhance, red-eye, basic
  exposure/white-balance/contrast and simple filters/presets.
- Per-item version history, undo, batch apply and originals preserved.
- External editor handoff to Affinity/Photoshop/Lightroom/Pixelmator with a
  round-trip version stack; do not build a full RAW editor.
- Validate RAW/HEIC orientation and basic color handling for edited previews.

**Acceptance:** edits never overwrite originals; every edit is undoable and
reproducible; metadata templates round-trip through XMP; a selection can
receive keywords and a preset without opening each item.
## 4. Tags, untagged media, and cleanup

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

## 5. Smart albums, curated albums, and organization

**Current:** system smart albums plus user smart albums with create/edit/delete
and rule builder. Curated albums (schema v19): File > New Album (⌘N), New
Album with Selection, rename (double-click or ⌘K) and delete with undo; the
Add to Album picker (⇧⌘A, the context menu) and Remove from Album (⌫ in an
album, the context menu) with undo, live member counts. Pin Album / Unpin
Album (⌘K) put albums and smart albums in a Pinned group at the top of the
sidebar; ⌘K Duplicate Smart Album copies the rules, and Export / Import Smart
Album Rules… save them as a versioned JSON file (an import whose rules do not
compile, or that comes from a newer build, is refused with a banner). In an
album, choosing a year on the year rail shows that year across the whole
library, as search does; Back returns to the album. Smart albums can match
the culling flag (Flag is / is not Pick, Reject or Unflagged), and the count
follows each flag key. Not yet: album covers,
drag to album, nesting, descriptions, manual reorder, and undo for pin.

**Target:**

- Drag to album (after 0.6).
- Cover/preview thumbnails (after 0.6).
- Nested folders/collections (manual reorder: after 0.6; albums follow the
  sort order).
- Album descriptions, custom icons, and optional colors.
- Richer smart rules: relative dates, camera/lens sets, rating/color ranges,
  location radius, media duration, and "similar to this item" (later).
- Smart-album empty state and explanation of why an item matched.

**Acceptance:** albums are discoverable, editable, and undoable; rules are
portable; counts update after watcher changes; no orphaned albums after item
deletion.

## 6. Multi-select and batch editing

**Current:** range/cmd selection, select-all, batch favorite/tags/trash/export,
and batch rating/color in the inspector. Culling keys (`P` / `X` / `U`,
`0`–`5`) flag or rate the whole selection with one undo entry; File > Delete
Rejected… (⌘K too) opens the Delete dialog on the open view's rejects only.
No rubber-band selection, mixed-value editing, batch
title/date/location/caption, or grouped undo.

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

## 7. Video player

**Current:** AVFoundation playback with play/pause, drag-scrub (press, drag,
release), frame stepping, speed menu, mute, loop, PiP and fullscreen. Hovering
or dragging the scrubber shows the time and the nearest frame from a preview
strip decoded in the background once per open video (up to 48 frames, so a
long video previews the nearest strip frame; the main picture shows the exact
seek). A file that cannot be previewed stays time-only. A playing video pauses
while scrubbing and resumes on release. Resume position is saved per video;
playback speed and mute are remembered across sessions. The immersive
fullscreen HUD hides after 2 s idle. Volume is fixed (there is no volume
control). No buffered range, chapters, subtitle/audio track selection, or loop
range. Not checked by hand: scrub previews on a 4K/HEVC file while playing.

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

## 8. Browsing, search, and viewer

**Current:** structured query grammar with negation (`-word`, `-tag:`,
`-is:favorite`, `-has:`), aliases (`is:untagged`, `is:unrated`,
`is:unlabeled`, `is:favourite`), and misspelled or unknown filters reported
as "Not understood" instead of being ignored; typed text never reaches FTS5 as
syntax ([docs/search.md](docs/search.md)). Search always covers the whole
library and moves the sidebar to Library; Back returns to the album. While the
field is focused, a footer explains the query in plain words. Suggestions
cover operators and tags; recent searches (10, per machine) are marked and
can be cleared from ⌘K. Quick filters in ⌘K (favorites, videos, rating,
colour, unrated, unlabeled, Clear Filters) edit the search text; nothing is
dimmed. ⌘K Save Search as Smart Album… keeps a search as a smart album. A
colour-label index (schema v15) keeps colour filters fast. Timeline buckets,
sorting, grid density, viewer zoom/pan and keyboard navigation as before.
Known limits: no value suggestions for cameras, lenses or cities; renaming a
tag does not rewrite `tag:` inside a saved search; an older app build reads a
saved search as an album with no rules (the whole library). No map or
jump-to-date (compare/survey and the slideshow came later, in plan 5 C1 and C2).

**Target:**

- Saved searches and recent search history, with a displayable query chip.
- Search suggestions/autocomplete for cameras, lenses, cities, tags, and
  structured operators.
- Quick filter chips for rating and color, not just favorite/video/raw/place.
- Compare view (2-up and 4-up), synchronized zoom/pan.
- Slideshow with interval, loop, and optional music-free presentation mode.
- Map view using the existing offline geocoder and GPS metadata.
- Duplicate detection and burst stacks with a review UI (shipped: Duplicates >
  Similar shots, plan 5 D2).
- Quick Look/Finder integration and drag-out to Finder.
- Native share sheet for the selected originals (shipped: File menu, ⌘K,
  context menu and batch dock Share…, macOS only; plan 5 D4).

**Acceptance:** search is discoverable without memorizing the grammar; compare
and slideshow are keyboard-operable; map/duplicate views have empty and error
states; no feature blocks the UI thread.

## 9. Everyday interaction and UX polish

Detailed audit: [docs/ux_audit.md](docs/ux_audit.md).

**Current:** dark graphite theme, status-bar feedback, error banner, preferences
(General/Storage/About), native menus, command palette, tooltips, shortcut
sheet, persisted window/layout/sort, Back/Forward history, and a restored
browsing context on relaunch (view, search, year, selection and grid scroll
position; dropped if the user acts during the startup scan; Quit saves it).
A toast (one at a time) with Undo for reversible actions; static skeleton
cards that the decoded thumbnail fades into, and a "No preview" card when a
thumbnail cannot be decoded; empty states with an icon, a sentence and an action
(Show all photos, Edit Rules…, Rescan, Choose Another Folder…, or removable
search terms). The viewer fades in on open and keeps the full stage while a
frame decodes; menus, dialogs and the palette fade in. Export and Import XMP
Metadata run off the UI thread with a status-bar line and a final toast.
Still missing: a background job center, per-item export progress and cancel,
a watcher summary (watcher changes are silent), shared-element viewer
open/close, item crossfade and animated grid reflow.

**Target:**

- **Selection and open contract:** single click selects, double click opens,
  Enter/Space opens, Escape returns; click on empty grid clears; Shift+arrow
  extends; hover/focus/pressed states are distinct.
- **Motion:** shared-element viewer open/close, item crossfade, overlay timing
  tokens, thumbnail fade-in, reduced-motion mode.
- **Navigation:** back/forward, clickable breadcrumbs, restored last
  album/search/selection/scroll, recent libraries and recent views.
- **Zoom:** pointer/keyboard-anchored grid zoom with animated reflow;
  cursor-anchored viewer zoom, pinch, fit/100/200 presets, zoom HUD. Shipped in
  0.4 (UX-2) except animated reflow; trackpad pinch not checked by hand, and
  momentum scroll and the month band still have known zoom issues.
- **Video:** drag-scrub with real frame previews, buffered range, resume
  position, auto-hide HUD, persisted speed/volume.
- **Feedback:** skeleton/error states, actionable empty states, toasts with
  undo, background job center for import/export/scan/backup.
- **System:** light/system theme, accent, density, motion settings, shortcut
  customization, expanded Preferences, tabs/recent views, diagnostics.

### UX passes

| Pass | Target | Focus |
| --- | --- | --- |
| **UX-1 Feel** | 0.3 | Selection/open semantics, view restore, skeletons/toasts, non-blocking jobs, motion foundation. |
| **UX-2 Precision** | 0.4 | Zoom anchors, video drag-scrub/resume, search/filter/tag polish, mixed-value batch editing. |
| **UX-3 Immersion** | 0.6 | Compare/slideshow/map, viewer quick edits/external editor, video tracks/chapters. |
| **UX-4 Personalization** | 0.7 | Theme/density/motion settings, shortcuts, preferences, accessibility/i18n. |

**Acceptance:** every interactive control has hover/active/focus/disabled states;
single click never opens and double click never merely selects; viewer open and
return preserve context; zoom anchors on the pointer; video scrubbing shows the
destination frame; errors are actionable; reduced motion is honored. The full
interaction audit and test plan are in [docs/ux_audit.md](docs/ux_audit.md).

## 10. Accessibility and localization

**Current:** keyboard-first navigation, command palette, native menu bar,
tooltips, shortcut sheet. GPUI now reports an AccessKit tree, but Lumina's
controls set no roles yet, so VoiceOver sees almost nothing beyond the window.
i18n catalog exists but ships en-US only.

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

## 11. Data safety, portability, and diagnostics

**Current:** SQLite migrations and backups, sidecars for title/rating/tags/caption,
rights, and edited date/location (written and imported), undo, single-instance
lock, logging, cache eviction. Global tag rename, merge and delete undo as one
step each (exact rows, aliases and smart-album rules; albums edited since are
kept). Rotation (R / ⇧R, ⌘K, Image menu) undoes as one step per batch, each
item back to its own angle. No backup/restore UI.

**Target:**

- Auto-import/export of XMP sidecars on scan as a preference.
- Catalog backup/restore UI and an integrity report.
- Library health check: missing files, unreadable sidecars, FTS consistency,
  stale cache.
- Undo coverage for rotation: done (one step per batch, see Current).
- Crash-safe recovery prompts after an interrupted import or migration.
- Diagnostics export and a clear privacy policy for logs.

**Acceptance:** a user can move a library to another machine with media +
sidecars + catalog backup and recover organization; no metadata is trapped in
a single database.

## 12. Performance, reliability, and scale

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

## 13. Platform and release

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

## 14. Local intelligence: people, OCR, memories and semantic search (down)

**Current:** not implemented. Structured and FTS search only.

These are deliberately down the release roadmap. They must be on-device,
opt-in, revocable and deletable; no cloud inference and no network upload.

**People and pets:**

- Local face/pet clustering with user-assigned names, merge/split and
  privacy controls. Index is opt-in and can be deleted.

**OCR and object/scene indexing:**

- OCR screenshots/documents for text search.
- Local object/scene tags for queries such as "beach", "dog" or "receipt".

**Memories and auto-curation:**

- "This day", trips, best-of-month and similar smart collections.
- Slideshow/memory presentation and controls over what gets surfaced.

**Semantic search:**

- On-device embedding model (size, license, quality, languages).
- Vector storage and incremental indexing in SQLite; no network.
- Hybrid ranking with structured/FTS filters; explainable results.
- A clear "search by meaning" entry point, result provenance, and the ability
  to disable/delete embeddings.

**Acceptance:** a design document with model/size/privacy tradeoffs is approved
before implementation; offline tests prove indexing, ranking and memory limits;
the user can delete every index and cache.
## 15. Optional private backup to S3-compatible storage (mid, 0.5)

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

| Priority | Band | Item | Milestone | Notes |
| --- | --- | --- | --- | --- |
| P0 | Top | Signed/notarized release + CI validation | 0.2.x | Needs Apple credentials and a native CI run. |
| P0 | Top | Onboarding + import/copy + import templates/verification | 0.3 | Core new-user and data-ingestion path. |
| P0 | Top | Migration importers (Photos, Takeout, Lightroom) | 0.3 | Removes switching friction. |
| P0 | Top | Selection/open/hover states + motion foundation | 0.3 | UX-1; single click selects, double click opens. Shipped: selection/open semantics, motion tokens, overlay/toast/thumbnail fades. Missing: shared-element viewer motion, item crossfade. |
| P0 | Top | Back/forward, restore last view/selection/scroll | 0.3 | UX-1 navigation continuity. Shipped (Go menu, ⌘[ / ⌘], ⌘K; restore on relaunch). Missing: recent views, clickable breadcrumbs. |
| P0 | Top | Skeletons, actionable empty states, toasts/undo, background jobs | 0.3 | UX-1 feedback and orientation. Shipped: skeletons, "No preview" cards, actionable empty states, toast with Undo, export and XMP import off the UI thread. Missing: job center, per-item progress/cancel, watcher summary. |
| P0 | Top | Grid/viewer zoom anchoring + pinch/presets | 0.4 | UX-2 precision browsing. Shipped: pinch, ⌘-scroll zoom and scroll-to-pan in the viewer; ⌘0–⌘4 presets with Fill and preset buttons; one anchored grid ladder for wheel, pinch and keys. Missing: animated grid reflow. Not checked on a real trackpad. |
| P0 | Top | Video drag-scrub with real previews + resume | 0.4 | UX-2 everyday media controls. Shipped: drag-scrub, strip frame previews, pause while scrubbing, resume position, remembered speed and mute. Missing: buffered range. Not checked by hand on a 4K/HEVC file. |
| P0 | Top | Search/filter/tag/smart-album polish | 0.4 | UX-2 correctness and discoverability. Shipped (search): negation, aliases, reported unknown filters, query explanation, recent-search markers and clearing, quick filters as query tokens, whole-library scope, Save Search as Smart Album, colour-label index. Missing: value suggestions (cameras, lenses, cities), tag and smart-album polish, jump-to-date. |
| P0 | Top | Hierarchical keywords + IPTC/XMP metadata templates | 0.4 | Creator/rights/contact/keywords; sidecar round-trip. |
| P0 | Top | Quick non-destructive edits + external editor handoff | 0.4 | Crop/rotate/auto-enhance; version history. |
| P0 | Top | Batch metadata + grouped undo | 0.4 | Apply templates/keywords to a selection. |
| P1 | Mid | Private S3-compatible backup (encrypted, OS credentials) | 0.5 | User-owned bucket; no sync/browsing. |
| P1 | Mid | Curated albums + smart-album polish | 0.6 | Shipped: curated albums (⌘N, ⇧⌘A picker, ⌫ in an album, undo), pin for albums and smart albums, Duplicate Smart Album, rule JSON export/import. Missing: covers, drag to album, smart-album preview of why an item matched. |
| P1 | Mid | Culling: pick/reject flags, compare/survey | 0.6 | Keyboard-first culling. Shipped: pick/reject flags (schema v20), P/X/U and 0–5 in grid and viewer with auto-advance, `is:pick`/`is:reject`/`is:unflagged`, Show Picks / Show Rejected, Flag smart-album rule, Delete Rejected…, compare/survey of 2–9 items with shared zoom (C, View > Compare; plan 5 C1). |
| P1 | Mid | Compare, slideshow and map | 0.6 | UX-3 presentation and geography. Shipped: compare/survey of 2–9 items (C1); slideshow (C2: ⌥⌘S, View > Slideshow, ⌘K; 3/5/10 s interval, Loop, Space/←/→/Esc, video posters, no fade under Reduce Motion). Missing: map; slideshow music and transitions beyond a fade. |
| P1 | Mid | Video tracks/chapters/loop range/trim | 0.6 | UX-3 after everyday scrubbing. |
| P1 | Mid | Duplicate detection and stacks | 0.6 | Large-library cleanup. Shipped: exact-copy merge; Similar shots tab in Duplicates (plan 5 D1/D2) for RAW+JPEG pairs and bursts, with Keep picks / Reject all but the first (flags only, one undo; Delete Rejected… removes them), loading, empty and error states. Missing: near-duplicate (visual) matching, collapsing stacks in the grid. |
| P1 | Mid | Share sheet and export presets | 0.6 | Distribution workflows. Shipped: Export Originals… and Export as JPEG… (2048 px, edits applied, no metadata; plan 5 D3); macOS share sheet for the selected originals from the File menu, ⌘K, the context menu and the batch dock (plan 5 D4). Missing: export cancel control. |
| P1 | Mid | RAW/HEIC validation + basic color handling | 0.6 | Table stakes for photographer credibility. |
| P1 | Mid | Light/system theme, accent, density, motion settings | 0.7 | UX-4 consistency. |
| P1 | Mid | Accessibility bridge, focus order, reduced motion | 0.7 | Grade A requirement. |
| P1 | Mid | Locale catalog (one non-English locale) | 0.7 | i18n proof. |
| P2 | Lower | Windows/Linux port | 0.8 | Large; follow RELEASING plan. |
| P2 | Lower | In-app updates and release notes | 0.7+ | Requires feed/signing infrastructure. |
| P2 | Lower | Diagnostics bundle and health check | 0.7 | Support quality. |
| P2 | Lower | Custom keyboard shortcuts | 0.7+ | Power-user polish. |
| P2 | Lower | Batch rename/file move | 0.6+ | Higher risk; dry-run first. |
| P2 | Lower | Quick Look/Finder extension | 0.6+ | Nice system integration. |
| P2 | Down | People and pets clustering | 1.1 | On-device, opt-in, deletable. |
| P2 | Down | OCR/object indexing | 1.1 | Local only. |
| P2 | Down | Memories and auto-curation | 1.1 | Depends on indexing. |
| P2 | Down | Semantic search | 1.2 | Design spike first. |

## Out of scope for the near term

- Google Photos, iCloud Photos, Dropbox, OneDrive and Google Drive account
  integration, remote library browsing and two-way sync. Optional S3-compatible
  backup (section 15) is separate and mid-roadmap.
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