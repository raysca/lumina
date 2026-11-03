# Download queue for URL video import

Status: draft for review (2026-11-03). It builds on the network video import
in `net_import` (see `docs/network-import.md`).

Decisions are marked **(user)** when the user made them in the design
conversation and **(call)** when the designer made them. Anything marked
(call) is open to change.

## Why

Today "Import Video from URL" is a modal that runs one download at a time:
Input → Inspecting → Ready (confirm) → Downloading. The modal stays up for the
whole download, and nothing else can be queued. Importing several videos means
waiting for each to finish before pasting the next.

The user wants it to work like downloads in a browser. You paste a link, and
it goes into a list that downloads in the background while you keep working.
You can open or close the list, remove items so the next one moves up, and see
what finished.

## Goals

- Pressing Import closes the dialog immediately and adds the video to the
  download list, so the next link can be pasted at once **(user)**.
- Up to 3 downloads run at the same time. The rest wait in the order they were
  added **(user)**.
- A toolbar icon shows a count of downloading plus waiting items and opens a
  dropdown list, browser-style (mockup option A) **(user)**.
- Removing a waiting or running item moves the next one up **(user)**.
- Finished downloads stay in the list as a record **(user)**.
- A built-in "Downloaded" album in the sidebar shows all media imported from a
  URL **(user)**.
- Quitting with running downloads asks first. Unfinished items come back
  paused when Lumina reopens **(user, agreed)**.
- Failures stay in the list with a plain reason and Retry, and the icon shows
  a red dot until the list is opened **(user, agreed)**.
- Duplicate URLs are handled as described under "Duplicates" **(user, agreed)**.
- The UI never blocks. Inspecting, downloading, cataloguing and every queue
  write stay off the UI thread.

## Non-goals

- Reordering waiting items by drag or by keyboard **(user: "no dragging")**.
- Resuming a partial download. A resumed item restarts from zero **(user,
  agreed)**.
- Pausing a running download by hand. "Paused" exists only for items
  interrupted by quitting **(call)**.
- A setting for the number of simultaneous downloads. It stays fixed at 3
  **(call)**.
- Dock badge, system notifications, download speed limits, scheduling.

## Anti-goals

- Anything that starts network traffic without the user asking. Paused items
  never start on their own at launch, which keeps the §5.4 promise of no
  background network.
- Losing a finished download because the queue bookkeeping failed. The file on
  disk and its catalog row matter more than the list row.
- Naming the download tools in the UI. Messages say what happened and what to
  do (see the memory rule and §5.4).

## The experience

### Adding

1. File > Import Video from URL (and its existing shortcut and palette entry)
   opens the dialog. It no longer refuses while a download is running. It
   still refuses with no library open, or while another modal is open
   **(call)**.
2. The user pastes a link and presses Import or ⏎.
3. Checks that need no I/O, such as an empty field or an unsupported URL, show
   an inline error in the dialog as today (`classify_url`).
4. A background lookup of the raw URL in `media_sources` runs (see
   Duplicates). When nothing matches, the dialog closes and a row appears in
   the list under Downloading or Waiting. The icon count goes up. If the
   dropdown is closed it stays closed **(call)**; the count change is the
   feedback.
5. The current Ready confirmation step and its Favorite toggle go away. The
   user can mark the video a favorite after it lands **(call)**.

### The toolbar icon

- A download icon in the toolbar, placed before Sort/Tags/Info **(call: exact
  position)**. It has a tooltip "Downloads" and a stable `debug_selector`.
- Badge: the number of downloading plus waiting items. It is hidden when that
  number is 0.
- A progress ring around the icon shows the combined progress of running
  items, using bytes when sizes are known and otherwise no ring **(call)**.
- A red dot appears when an item has failed since the list was last opened.
- The icon is shown only once the library has any download rows (any state),
  so a library that never used URL import stays as it is today **(call)**.

### The dropdown

An overlay anchored under the icon (occluding card plus dismiss backdrop, per
§6.3), about 320 px wide, with a scrollable body. It has these sections, each
shown only when it has items:

- **Downloading:** title (the URL until the title is known), progress bar,
  "142 of 380 MB · 2 min left" or a phase message ("Getting ready…",
  "Finishing…"), and ✕ to cancel.
- **Waiting:** title or URL, "Next up" on the first one, and ✕ to remove.
- **Paused:** interrupted by quitting, with Resume and ✕. When there is more
  than one, the section header has "Resume all".
- **Failed:** title, a plain reason, Retry, and ✕ to remove the row.
- **Done:** title and "Added to library". Clicking the row opens the
  Downloaded album and selects the item **(call)**. If the item has since been
  deleted, the row says "Removed from library" and is not clickable.
- Footer: "Clear finished" removes Done and Failed rows **(call: Failed
  included)**, and "Show Downloaded" opens the album.

Row thumbnails are not part of this change. Rows are text and progress only
**(call)**.

Removing a row never touches the media in the library. ✕ on a running item
cancels it and deletes its partial file, and the next waiting item starts.

### Keyboard (§11)

- A Show/Hide Downloads action with shortcut ⌥⌘L, matching Safari **(call)**.
  It also goes in the command palette, in the View menu, and in the shortcut
  sheet.
- In the open dropdown: ↑/↓ move a visible focus ring between rows; ⏎ does the
  row's main action (Show, Retry, Resume); ⌫ removes or cancels; Esc closes.
  The dropdown absorbs other shortcuts while open (§6.3).

### Quitting and reopening

- Quit (`QuitApp`: ⌘Q and the menu item, the only path that calls
  `cx.quit()` today) with running or waiting
  items shows a confirm: "2 downloads are still running. Quit anyway?" with
  **Quit** and **Cancel**. On Quit, running items are cancelled and stored as
  Paused, waiting items are stored as Paused too, and partial files are
  deleted.
- Opening a different library (`open_library`; there is no "close library")
  with active downloads uses the same confirm **(call)**. The queue belongs to
  the library (see Data).
- Closing the window keeps today's behaviour, and this change adds no
  should-close hook **(call)**.
- Quits that the system starts (logout, Dock > Quit) can't be intercepted in
  GPUI 0.2.2. They go through crash recovery instead: no confirm, and rows
  load as Paused at the next launch **(call)**.
- On reopen, Paused items appear in the list and nothing starts until the user
  presses Resume or Resume all. The badge counts Paused items too **(call)**,
  so the user sees there is unfinished work.
- If Lumina crashed, any row stored as running or waiting is loaded as Paused,
  and leftover partial files are cleaned up **(call)**.

### Failures

- The reason is the existing user-facing error text from `net_import` (the top
  `anyhow` message, already written for people). The chain still goes to the
  log, as today.
- Retry moves the row to the end of Waiting **(call)**.
- A download whose required helper is missing fails with the existing
  missing-helper message, with no Retry **(call)**.
- "Saved but couldn't be added to Lumina" (today's `NotCatalogued`) becomes a
  Failed row with today's message (the file is kept and a rescan adds it).
  It has no Retry, so the video is not downloaded twice **(call)**.
- A timeline refresh error after a successful import keeps today's banner, and
  the row is Done.

### Duplicates

- **Already in the list as Waiting, Downloading or Paused:** the item is not
  added. The dialog closes, the dropdown opens, and the existing row is briefly
  highlighted with "Already in your downloads" **(user)**.
- **Downloaded before and still in the library:** the dialog stays open and
  says "You downloaded this on <date>." with **Show in Library** (default) and
  **Download Again** **(user)**. This uses the raw URL against
  `media_sources.source_url` (already indexed) plus `imported_epoch_ms`, and
  matches only rows Lumina downloaded. A sidecar-only source never triggers
  the prompt, so the date is always a real download date **(follows from the
  user's answer)**.
- **Downloaded before but now trashed or gone:** download normally with no
  prompt **(user)**. `find_media_by_source_url` already ignores trashed items.
- **Canonical URL match found only after inspection** (for example
  `youtu.be/x` versus `youtube.com/watch?v=x`): the row moves to an "Already in
  your library" state with Show and Download Again, instead of downloading
  **(call)**.

### Downloaded album

- A new `SystemAlbum::Downloaded` sits in the sidebar's Library section after
  Videos **(call: position)**. It contains non-trashed media that Lumina
  downloaded from a URL. A web address that only came from an imported XMP
  sidecar does not count **(user)**; see "Telling downloads apart" under Data.
  Its count comes from the existing system album counts.
- It uses the same SQL synthesis path as the other system albums, and is
  checked with `EXPLAIN QUERY PLAN` (§10).
- It is hidden when empty **(call)**, the same as Places or Missing if they do
  that today (follow the existing convention).

## Data

The queue is stored per library in the catalog, so it survives restarts and
travels with the library. This needs a new table, a migration and a
`user_version` bump (§5.3):

```
download_queue(
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,          -- as pasted
  canonical_url TEXT,         -- NetworkMediaInfo.url once inspected
  title TEXT,
  state TEXT NOT NULL,        -- waiting | running | paused | failed | done | duplicate
  error TEXT,                 -- user-facing reason when failed
  media_id INTEGER,           -- set when done; NULL if the item was purged
  added_epoch_ms INTEGER NOT NULL,
  finished_epoch_ms INTEGER
)
```

The exact column set is up to the builder. Rules:

- The waiting order is `added_epoch_ms, id`, so Retry re-stamps
  `added_epoch_ms`.
- Progress is in memory only and is never written per tick.
- Every state change is a catalog write made off the UI thread. A failed write
  is logged and shown in the error banner, and the in-memory row is reverted
  (§2, §5.2). A failed write never deletes or hides a downloaded file.
- Done rows stay until "Clear finished". No automatic pruning **(call)**.
- `media_id` must not block media purge (use no FK, or `ON DELETE SET NULL`).

### Telling downloads apart

`media_sources` today gets rows from two places:
`ingest_network_imported_file` (a real download) and XMP sidecar import when
`dc:source` is an http(s) address (`lib.rs`, sidecar import). Only the first
counts as Downloaded **(user)**.

- Add `downloaded INTEGER NOT NULL DEFAULT 0` to `media_sources`. This goes in
  the same migration and `user_version` bump as `download_queue`. Downloads set
  it to 1; sidecar import leaves it at 0 **(call)**.
- A sidecar import onto an item that was already downloaded must not reset the
  flag to 0. Check how `record_source_in` writes (an upsert must keep
  `downloaded = MAX(old, new)`), and cover it with a test.
- Backfill for existing v13 rows: set `downloaded = 1` where the media's
  `file_path` is under the library's `Imports/` folder, the only place
  downloads land. Leave others at 0 **(call)**. This heuristic can mislabel a
  sidecar-sourced file that was copied in by local import, which is an
  acceptable, rare miss.
- `find_media_by_source_url` (the duplicate check) and the Downloaded album
  both filter on `downloaded = 1`. Check the album query with
  `EXPLAIN QUERY PLAN`, and add an index only if the plan needs one.

## Architecture

- **Scheduler (UI crate).** A `DownloadQueue` state struct replaces
  `NetworkImportStage::Inspecting/Ready/Downloading`. It holds the rows, the
  per-row cancel flags (`Arc<AtomicBool>`, as today) and the in-memory
  progress. When a slot frees up, or a row is added or resumed, it starts the
  oldest Waiting rows until 3 are running. It lives in the GPUI app, not in
  core, because it only coordinates UI-visible state **(call)**.
- **A job is inspect, then download, then ingest.** It reuses
  `inspect_url`, `download_network_media` and `ingest_network_imported_file`
  unchanged, on its own named thread (as today), reporting through the
  existing `ProgressCallback`. Inspection counts against the 3 slots
  **(call)**.
- **Core gets queue persistence and one concurrency fix.** It adds catalog
  functions to insert, update the state of, list and clear queue rows, plus
  the Downloaded system album. The signatures of `net_import`'s download and
  ingest functions stay as they are.
- **Stale staging cleanup moves to library open (required).** Today both
  `download_network_media` and `plan_import` call `remove_stale_temps`, which
  deletes every `.*.lumina-import` staging entry under `Imports/`. The
  assumption behind it is "only one import runs at a time". With parallel
  jobs, each new download, or a local import, would delete the staging files
  of the jobs already running. The cleanup runs once when a library opens,
  before any job can start, and is removed from both per-job paths. This
  needs a regression test: two concurrent jobs, where the second starting
  does not remove the first's staging.
- **Destination names are already collision-safe.** `claim_name` uses
  `hard_link`, which fails when the name exists, and staging folders are named
  by nanosecond timestamp. Keep it that way.
- **Progress to UI.** Use one channel per job, as today, or one shared channel
  tagged with the row id. Either way, UI updates are coalesced to the frame,
  not one notify per callback.
- **The local import dialog and URL downloads coexist.** Neither blocks the
  other any more **(call)**. Both go through the serialized catalog
  connection.
- **Opening another library** goes through the same confirm-and-pause path as
  quit.

## Testing (§6)

- Core: migration from the current schema version (idempotent), queue CRUD
  and ordering, crash recovery (running→paused), the Downloaded album query,
  and its query plan.
- UI automation:
  - pressing Import closes the dialog and adds a row;
  - a 4th item waits, and cancelling one starts it;
  - ✕ on Waiting, Failed and Done rows;
  - Retry;
  - duplicate cases: in the queue, in the library, trashed;
  - keyboard open and close plus arrow, ⏎, ⌫ and Esc in the dropdown;
  - badge count and red dot;
  - the quit confirm;
  - Paused items do not start at launch.
- Jobs are driven by a test seam that replaces the real inspect/download
  (deterministic, no network), so concurrency is tested without sleeps.
- Existing `tests/network_import.rs` tests are updated for the new flow.
- Live check: one real download plus a queue of 4 in `make dev`, and UI smoke.
  Report anything skipped.

## Left to the builder

- Icon art, exact spacing and colours (follow `theme` and the toolbar icon
  buttons).
- The exact table and column names, and whether progress uses one channel or
  several.
- How the job test seam is shaped (smallest that works).
- Module split: keep `network_import.rs` and the dialog under about 500 lines
  each (§5.1); the dropdown is its own view file.
