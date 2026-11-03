# Download Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn URL video import into a browser-style download queue: up to 3
at once, a toolbar icon with a count, a dropdown list, and a Downloaded album.
Unfinished items survive quit as Paused.

**Architecture:**
- **Core:** stores the queue (`download_queue` table) and tells real downloads
  apart from sidecar sources (`media_sources.downloaded`). It also moves the
  stale staging cleanup to library open, so parallel jobs stop deleting each
  other's files.
- **UI crate:** a `DownloadQueue` scheduler runs each job on its own thread
  through a swappable job function, so tests never touch the network. The
  dialog shrinks to URL entry plus the "downloaded before" prompt.

**Tech Stack:** Rust 2021, GPUI 0.2.2, rusqlite, anyhow, async-channel
(already used), chrono (already a UI dependency).

**Spec:** `docs/superpowers/specs/2026-11-03-download-queue-design.md`. Read it
before any task; this plan doesn't repeat its UX copy or rationale.

## Global Constraints

- Follow `AGENTS.md`. In particular, gates before every commit:
  `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo test --workspace`.
- One logical change per commit, with a `feat:`/`fix:`/`test:`/`docs:` prefix
  and a body stating what was verified. End every commit message with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Don't push.
- No new dependencies.
- UI copy never names yt-dlp, ffmpeg, curl, Homebrew or HLS.
- No catalog write on the UI thread. On a failed write: `log::warn!`/`error!`,
  show the error banner (`set_error`), and revert the in-memory row.
- Tests: in UI test modules use `#[core::prelude::v1::test]`, temp libraries
  via `seed_library`/`app_with_library`, no sleeps, no real network. Use
  `secondary-` in keystrokes, never `cmd-`.
- Every new interactive control gets a stable `debug_selector`. Names are
  fixed per task below.
- `MAX_RUNNING = 3`, fixed.
- The waiting order is `added_epoch_ms ASC, id ASC`.
- Shortcut ⌥⌘L toggles the dropdown.
- Schema version becomes 14. Task 2 bumps it, and Task 3 adds to the same
  version: both are unreleased, and the migrations are idempotent
  `IF NOT EXISTS` / column checks.

## Review Focus

- **A job that finishes after its row was removed, or after a different
   library was opened.** It must not bring the row back or write to the new
   library's catalog. Test in Task 4 (`finished_job_for_removed_row_is_ignored`)
   and Task 6 (`switching_library_drops_old_jobs`).
- **The same URL submitted twice quickly** (the second press lands before the
  first row's insert returns): exactly one row. Test in Task 4
  (`double_submit_adds_one_row`).
- **Cancel during the inspect phase, before any progress arrives:** the row
  goes away and the next waiting item starts. Test in Task 4
  (`cancel_while_inspecting_starts_next`).
- **An existing v13 catalog with download and sidecar sources:** after the
  upgrade only Imports/ files count as Downloaded, and a second open changes
  nothing. Test in Task 2 (`v13_sources_backfill_downloaded_flag`).
- **Many rows and very long titles or URLs in the dropdown:** they truncate,
  the list scrolls, and the card stays within the window. Test in Task 5
  (`dropdown_truncates_and_scrolls`).

---

### Task 1: Staging cleanup runs once at library open

**Files:**
- Modify: `crates/lumina-core/src/net_import/mod.rs:393-395` (remove the
  `remove_stale_temps` call and its "only one import" comment)
- Modify: `crates/lumina-core/src/import.rs:73` (remove the call from
  `plan_import`)
- Modify: `crates/lumina-core/src/lib.rs` (`open_with_paths`): call
  `crate::import::remove_stale_temps(&library_root.join(IMPORTS_DIR))` once,
  after the catalog opens and before the watcher starts
- Test: `crates/lumina-core/tests/net_import_test.rs`

**Interfaces:**
- Produces: no new API. `download_network_media` and `plan_import` no longer
  delete staging.

- [ ] **Step 1: Write the failing test** `starting_a_job_keeps_other_jobs_staging`:
  - Open an engine on a temp library.
  - Create `Imports/2026/2026-11-03/.x.lumina-import/part.mp4` by hand, as an
    in-flight job's staging.
  - Call `engine.plan_import(&[some_temp_file], &AtomicBool::new(false))`.
  - Assert the staging file still exists: "starting an import must not delete
    another job's staging".
- [ ] **Step 2: Write** `stale_staging_is_removed_when_the_library_opens`:
  - Create the same staging entry, drop the engine, and reopen it with
    `open_with_paths`.
  - Assert the entry is gone.
- [ ] **Step 3: Run**
  `cargo test -p lumina-core --test net_import_test staging`.
  Expected: the first test FAILS (the staging is deleted) and the second
  passes or fails, depending on the order.
- [ ] **Step 4: Move the call** as listed under Files.
- [ ] **Step 5: Run the same command.** Expected: both PASS.
- [ ] **Step 6: Run the gates, then commit**
  `fix: clean stale import staging only when a library opens`.

### Task 2: Mark real downloads, prior-download lookup, Downloaded album

**Files:**
- Modify: `crates/lumina-core/src/catalog/schema.rs` (v13 `media_sources`
  block ~l.145; migration section ~l.215; version stamp ~l.500)
- Modify: `crates/lumina-core/src/catalog/mod.rs:24` (`SCHEMA_VERSION = 14`)
- Modify: `crates/lumina-core/src/catalog/media.rs:786-832`
  (`record_media_source`, `record_source_in`, `insert_with_source`,
  `find_media_by_source_url`)
- Modify: `crates/lumina-core/src/net_import/mod.rs:501-504` (the engine
  wrapper)
- Modify: `crates/lumina-core/src/smart_albums/system.rs` (new variant)
- Modify: callers of `find_media_by_source_url`:
  `crates/lumina-ui-gpui/src/app/network_import.rs`,
  `crates/lumina-core/tests/net_import_test.rs:227`
- Test: unit tests in `catalog/media.rs` beside the v13 migration tests
  (~l.900-1050); a plan test in `smart_albums/system.rs` or its test module

**Interfaces:**
- Produces:
  - `pub struct PriorDownload { pub media_id: MediaId, pub path: PathBuf, pub imported_epoch_ms: i64 }`,
    exported from `lumina_core` (next to `NetworkMediaInfo`).
  - `Catalog::find_prior_download(&self, source_url: &str) -> Result<Option<PriorDownload>>`
    matches only `downloaded = 1` rows of non-trashed media. It replaces
    `find_media_by_source_url`; delete the old name.
  - `LuminaEngine::find_prior_download(&self, url: &str) -> Result<Option<PriorDownload>>`.
  - `SystemAlbum::Downloaded`: title "Downloaded", icon name
    `"arrow-down-to-line"`. Add it to `all()` after `Videos`. Its where clause
    is
    `EXISTS (SELECT 1 FROM media_sources s WHERE s.media_id = m.id AND s.downloaded = 1) AND m.is_trashed = 0 AND m.is_hidden = 0`.
  - `record_source_in(conn, media_id, source_url, uploader, downloaded: bool)`.
    `insert_with_source` passes `true` and `record_media_source` (sidecar)
    passes `false`. The upsert sets
    `downloaded = MAX(media_sources.downloaded, excluded.downloaded)`.
  - Exhaustive matches on `SystemAlbum` in the UI crate (`config.rs`
    `LastView` and others the compiler flags) need an arm. Map `Downloaded`
    to the default view in `LastView` unless an existing pattern says
    otherwise.

- [ ] **Step 1: Write the failing tests:**
  - `download_marks_source_downloaded`: `insert_with_source` leads to
    `find_prior_download(url)` being `Some` with the right `media_id`.
  - `sidecar_source_is_not_a_download`: `record_media_source` on a plain item
    leads to `find_prior_download(url)` being `None`.
  - `sidecar_import_keeps_download_flag`: insert with a source, then call
    `record_media_source` on the same id. It is still `Some`.
  - `trashed_download_is_not_prior`: insert, then trash it. The result is
    `None`.
  - `v13_sources_backfill_downloaded_flag`:
    - Build a catalog and insert two items, one under `<lib>/Imports/2026/…`
      and one at `<lib>/a.mp4`, both with source rows.
    - Then run
      `ALTER TABLE media_sources DROP COLUMN downloaded; PRAGMA user_version = 13;`.
    - Reopen twice. Only the Imports item is a prior download, and
      `schema_version() == 14`.
  - `downloaded_album_counts_only_downloads`: one downloaded item, one with a
    sidecar source and one plain item. Assert
    `query_system_album_count(SystemAlbum::Downloaded) == 1`.
  - `downloaded_album_query_plan`: run `explain_query_plan` on the synthesized
    query and assert no `SCAN media_sources` (a `SEARCH` by the primary key is
    fine).
- [ ] **Step 2: Run** `cargo test -p lumina-core downloaded` and
  `cargo test -p lumina-core prior`. Expected: FAIL to compile (the names
  don't exist).
- [ ] **Step 3: Implement:**
  - The column goes in the `CREATE TABLE` for new catalogs, plus a
    `pragma_table_info` check and `ALTER TABLE media_sources ADD COLUMN downloaded INTEGER NOT NULL DEFAULT 0`
    for existing ones.
  - Backfill once, only when the column was just added:
    `UPDATE media_sources SET downloaded = 1 WHERE media_id IN (SELECT id FROM media_items WHERE file_path LIKE ?1 ESCAPE '\')`.
    Bind `<library_root>/Imports/%` with LIKE escaping. If `Catalog` has no
    library root, pass it in from `open_with_paths`, or run the backfill from
    the engine right after `Catalog::open`. Either is fine; keep it in one
    transaction.
  - Unlike the existing `let _ =` ALTERs, propagate the error.
- [ ] **Step 4: Run the tests from Step 2.** Expected: PASS. Then run
  `cargo test --workspace` to catch the renamed callers.
- [ ] **Step 5: Run the gates, then commit**
  `feat: tell URL downloads apart from sidecar sources and add a Downloaded album`.

### Task 3: Persisted download queue

**Files:**
- Create: `crates/lumina-core/src/catalog/downloads.rs`
- Modify: `crates/lumina-core/src/catalog/mod.rs` (`mod downloads; pub use downloads::*;`)
- Modify: `crates/lumina-core/src/catalog/schema.rs`: add
  `CREATE TABLE IF NOT EXISTS download_queue (...)` with the columns below,
  and no foreign key to `media_items`
- Modify: `crates/lumina-core/src/lib.rs` (re-export the types)
- Test: unit tests in `downloads.rs`

**Interfaces:**
- Produces:
  ```rust
  pub enum DownloadState { Waiting, Running, Paused, Failed, Done, Duplicate } // stored as lowercase text
  pub struct DownloadRow {
      pub id: i64,
      pub url: String,
      pub canonical_url: Option<String>,
      pub title: Option<String>,
      pub state: DownloadState,
      pub error: Option<String>,
      pub can_retry: bool,
      pub media_id: Option<MediaId>,
      pub added_epoch_ms: i64,
      pub finished_epoch_ms: Option<i64>,
      /// Computed on read: media_id points at a non-trashed item.
      pub in_library: bool,
  }
  impl Catalog {
      pub fn add_download(&self, url: &str) -> Result<DownloadRow>;         // state Waiting, added now
      pub fn update_download(&self, row: &DownloadRow) -> Result<()>;       // writes every column but id/added/url
      pub fn requeue_download(&self, id: i64) -> Result<DownloadRow>;       // Waiting, error NULL, can_retry 1, added = now
      pub fn remove_download(&self, id: i64) -> Result<()>;
      pub fn clear_finished_downloads(&self) -> Result<usize>;              // Done | Failed | Duplicate
      pub fn list_downloads(&self) -> Result<Vec<DownloadRow>>;             // ORDER BY added_epoch_ms, id; LEFT JOIN media_items for in_library
      pub fn recover_downloads(&self) -> Result<usize>;                     // Running | Waiting -> Paused
  }
  ```
  `requeue_download` re-stamps `added` to
  `max(now, MAX(added_epoch_ms) + 1)`, so Retry always lands last even within
  one millisecond.

- [ ] **Step 1: Write the failing tests:**
  - `downloads_list_in_added_order`
  - `requeue_moves_to_the_end_and_clears_error`
  - `clear_finished_keeps_active_rows`: one of each state, and only Waiting,
    Running and Paused remain.
  - `recover_pauses_running_and_waiting`
  - `in_library_false_after_trash_and_purge`
  - `download_queue_survives_reopen` (file-backed catalog)
- [ ] **Step 2: Run** `cargo test -p lumina-core downloads`. Expected: FAIL to
  compile.
- [ ] **Step 3: Implement.** Each mutation is a single statement or a
  transaction.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Run the gates, then commit**
  `feat: store the URL download queue in the catalog`.

### Task 4: Queue scheduler, job seam, simplified dialog

**Files:**
- Create: `crates/lumina-ui-gpui/src/app/downloads/mod.rs` (state, scheduler,
  app methods)
- Create: `crates/lumina-ui-gpui/src/app/downloads/job.rs` (job types and
  `real_job`)
- Modify: `crates/lumina-ui-gpui/src/app/network_import.rs`:
  - Keep only the dialog stages.
  - Delete the `Inspecting`, `Ready`, `Downloading` and `MissingHelper`
    stages, the `cancel_flag`, `Outcome`, `confirm_network_download`,
    `toggle_network_import_favorite` and the two-Esc logic in
    `dismiss_network_import`.
  - Remove the `self.network_import.is_busy()` refusal in
    `open_network_import_dialog`.
- Modify: `crates/lumina-ui-gpui/src/views/network_import_dialog.rs`: render
  `Input`, `Checking`, `AlreadyDownloaded`.
- Modify: `crates/lumina-ui-gpui/src/app/keymap/modals.rs:103-135` (the
  network dialog keys)
- Modify: `crates/lumina-ui-gpui/src/app/state.rs`: add
  `pub downloads: DownloadQueue` to `LuminaApp`
- Test: rewrite `crates/lumina-ui-gpui/src/tests/network_import.rs`, and
  create `crates/lumina-ui-gpui/src/tests/downloads.rs`

**Interfaces:**
- Consumes: Task 2 `PriorDownload` and `find_prior_download`; Task 3
  `DownloadRow`, `DownloadState` and the catalog methods (through
  `engine.catalog()`); the existing `classify_url`, `helper_missing`,
  `inspect_url`, `download_network_media`, `ingest_network_imported_file` and
  `MISSING_PART`.
- Produces:
  ```rust
  // app/downloads/job.rs
  pub(crate) struct JobRequest { pub row_id: i64, pub url: String, pub kind: NetworkSourceKind, pub skip_prior_check: bool }
  pub(crate) enum JobEvent {
      Inspected { canonical_url: String, title: String },
      Progress(NetworkDownloadProgress),
  }
  pub(crate) enum JobOutcome {
      Imported { item: Box<MediaItem>, timeline_error: Option<String> },
      AlreadyInLibrary(PriorDownload),
      Cancelled,
      Failed(String),        // top anyhow message; chain logged
      MissingHelper,         // Failed row, can_retry = false, error = MISSING_PART
      NotCatalogued { path: PathBuf }, // Failed row, can_retry = false, today's message
  }
  pub(crate) type JobFn = Arc<dyn Fn(&JobRequest, &AtomicBool, &(dyn Fn(JobEvent) + Send + Sync)) -> JobOutcome + Send + Sync>;
  pub(crate) fn real_job(engine: Arc<LuminaEngine>) -> JobFn; // helper_missing -> inspect -> prior check -> download -> ingest(is_favorite = false)

  // app/downloads/mod.rs
  pub(crate) const MAX_RUNNING: usize = 3;
  pub(crate) struct DownloadEntry { pub row: DownloadRow, pub progress: Option<NetworkDownloadProgress>, pub skip_prior_check: bool, cancel: Arc<AtomicBool> }
  pub(crate) struct DownloadQueue {
      pub entries: Vec<DownloadEntry>,
      pub open: bool,                 // dropdown visible (Task 5)
      pub unseen_failure: bool,       // red dot (Task 5)
      pub focused: Option<i64>,       // row id with keyboard focus (Task 5)
      pub highlight: Option<i64>,     // "Already in your downloads"
      pub generation: u64,            // bumped on library change; stale job results are dropped
      pub job: Option<JobFn>,         // None until a library opens; tests replace it
  }
  impl LuminaApp {
      pub fn submit_network_url(&mut self, url: String, cx: &mut Context<Self>);   // dialog ⏎ / Import
      pub fn enqueue_download(&mut self, url: String, skip_prior_check: bool, cx: &mut Context<Self>);
      pub fn cancel_or_remove_download(&mut self, row_id: i64, cx: &mut Context<Self>); // ✕ and ⌫
      pub fn retry_download(&mut self, row_id: i64, cx: &mut Context<Self>);
      pub fn download_again(&mut self, row_id: i64, cx: &mut Context<Self>);         // Duplicate row -> Waiting, skip_prior_check
      pub(crate) fn pump_downloads(&mut self, cx: &mut Context<Self>);               // start Waiting until MAX_RUNNING running
      pub(crate) fn active_download_count(&self) -> usize;                           // Waiting + Running + Paused
      pub(crate) fn running_or_waiting_count(&self) -> usize;
  }
  ```
  Dialog stages become:
  ```rust
  pub(crate) enum NetworkImportStage {
      Input { url: String, error: Option<String> },
      Checking { url: String },                                  // raw-URL prior lookup off the UI thread
      AlreadyDownloaded { url: String, prior: PriorDownload },   // Task 4: Download Again + Cancel; Task 7 adds Show in Library as the ⏎ default
  }
  ```

**Scheduler rules** (these decide behavior the tests pin):
- **`submit_network_url`:**
  - On a `classify_url` error, show the inline error.
  - If an entry in Waiting, Running or Paused has the same `url` or
    `canonical_url`: close the dialog, set `open = true` and
    `highlight = Some(id)`, and add no row.
  - Otherwise go to `Checking`, then run `find_prior_download(raw)` in the
    background. On `Some`, show `AlreadyDownloaded`. On `None`, call
    `enqueue_download(url, false)` and close the dialog.
- **`enqueue_download`:** first insert a placeholder entry with id `-1`
  carrying the URL, so a second submit sees it. Then `add_download` runs in
  the background. When it returns, replace the placeholder and call
  `pump_downloads`. On error, drop the placeholder and show the error banner.
- **Each job:**
  - Runs on a `std::thread::Builder` thread named `lumina-download-<id>`.
  - Before starting, it persists `Running`.
  - Progress goes into one `async_channel` per job. The UI task drains every
    queued message with `try_recv` before one `cx.notify()`.
  - `Inspected` persists `canonical_url` and `title`.
  - Outcomes are persisted by a wrapper on the job thread before it reports to
    the UI:
    - `Imported` → `Done` with `media_id` and `finished`.
    - `AlreadyInLibrary` → `Duplicate` with `media_id`.
    - `Failed`, `MissingHelper` and `NotCatalogued` → `Failed` with the
      error and `can_retry`.
    - `Cancelled` → nothing is written.
- **UI side of an outcome:** if `generation` changed or the row id is no
  longer in `entries`, ignore it. Otherwise update the entry, set
  `unseen_failure` on a failure, show today's toast or banner for
  `Imported`/`timeline_error`, call `refresh_album_counts` and
  `refresh_active_view`, then `pump_downloads`.
- **`cancel_or_remove_download`:**
  - On a Running entry: set its cancel flag and remove the entry at once. The
    stale outcome is then ignored by the rule above.
  - On any state: `remove_download` in the background. On error, put the
    entry back and show the banner.
  - Then call `pump_downloads`.
- **Retry:** `requeue_download` in the background, then pump. On error, show
  the banner and leave the row as it was.

- [ ] **Step 1: Write the failing tests in `tests/downloads.rs`.**
  - A `fake_jobs(view)` helper sets `app.downloads.job` to a `JobFn`. The fake
    records each `JobRequest`, then blocks on a per-row
    `async_channel::Receiver<JobOutcome>` that the test controls, and returns
    `Cancelled` when cancelled.
  - Tests:
    - `submit_closes_dialog_and_adds_a_row`: also asserts
      `!app.downloads.open`, since the dropdown stays closed
    - `fourth_download_waits_until_a_slot_frees`: 4 submits give 3 Running
      and 1 Waiting. Finish one with `Imported`, and the 4th is Running.
    - `cancel_while_inspecting_starts_next`
    - `finished_job_for_removed_row_is_ignored`
    - `double_submit_adds_one_row`: two submits back to back with no run in
      between. `list_downloads().len() == 1`.
    - `same_url_in_queue_highlights_existing_row`
    - `prior_download_prompts_and_download_again_skips_check`: seed a
      downloaded item via `insert_with_source`. Submitting its URL shows
      `AlreadyDownloaded`. Download Again enqueues with
      `skip_prior_check == true`.
    - `trashed_prior_download_downloads_normally`
    - `canonical_duplicate_becomes_duplicate_row`: the fake returns
      `AlreadyInLibrary`, and the row state is `Duplicate`.
    - `failure_sets_red_dot_and_retry_requeues_last`
    - `missing_helper_failure_has_no_retry`
    - `rows_persist_states`: after each transition,
      `engine.catalog().list_downloads()` matches the in-memory states.
  - Use `run_until_parked()` between steps, never sleep.
- [ ] **Step 2: Rewrite `tests/network_import.rs`.**
  - Keep the input-validation, typing/backspace, clear-button, long-URL,
    palette and shortcut tests.
  - Change `network_import_shortcut_alt_cmd_u` to `"alt-secondary-u"`.
  - Delete the Ready, Downloading and favorite tests.
  - Add `dialog_opens_while_downloads_run`.
- [ ] **Step 3: Run** `cargo test -p lumina-ui-gpui downloads` and
  `cargo test -p lumina-ui-gpui network_import`. Expected: FAIL to compile.
- [ ] **Step 4: Implement the files listed above.** Set
  `downloads.job = Some(real_job(engine))` and bump `generation` in
  `open_library_inner` after the engine opens (Task 6 adds loading). Keep
  each new file under about 500 lines.
- [ ] **Step 5: Run the same commands.** Expected: PASS. Then run the full
  `cargo test -p lumina-ui-gpui`.
- [ ] **Step 6: Run the gates, then commit**
  `feat: queue URL downloads and run up to three at once`.

### Task 5: Toolbar icon and dropdown, with a keyboard path

**Files:**
- Create: `crates/lumina-ui-gpui/src/views/downloads_panel.rs` (the dropdown)
- Modify: `crates/lumina-ui-gpui/src/app/render/toolbar.rs`: add the icon
  button before `sort-toolbar-btn`, only when `!downloads.entries.is_empty()`
- Modify: `crates/lumina-ui-gpui/src/app/render/mod.rs`:
  - render the panel overlay;
  - add `("⌥⌘L", "Show or hide downloads")` to the shortcut sheet list near
    l.119;
  - add an `on_action` for the new action.
- Modify: `crates/lumina-ui-gpui/src/actions.rs` (`ToggleDownloads`),
  `menus.rs` (View: "Show Downloads", after Toggle Inspector)
- Modify: `crates/lumina-ui-gpui/src/app/command_palette.rs`:
  - add `CommandId::ToggleDownloads`, with key `command.toggle_downloads`,
    shortcut `⌥⌘L` and keywords `downloads queue progress url video`;
  - its handler calls `toggle_downloads_panel`.
- Modify: `crates/lumina-ui-gpui/src/i18n.rs`
  (`"command.toggle_downloads" => "Show Downloads"`)
- Modify: `crates/lumina-ui-gpui/src/app/keymap/mod.rs`: ⌥⌘L next to the ⌥⌘U
  handler
- Modify: `crates/lumina-ui-gpui/src/app/keymap/modals.rs`: when
  `downloads.open`, absorb keys:
  - ↑/↓ move `focused`;
  - ⏎ runs the row's main action;
  - ⌫ runs `cancel_or_remove_download`;
  - Esc closes the panel.
- Modify: `crates/lumina-ui-gpui/src/app/downloads/mod.rs`: add
  `toggle_downloads_panel`, which opening clears `unseen_failure`, and
  `clear_finished_downloads`
- Modify: `crates/lumina-ui-gpui/src/icons.rs`: add a download icon only if
  none fits. Reuse `Icon::ArrowDownToLine` if present.
- Test: `crates/lumina-ui-gpui/src/tests/downloads.rs` (extend)

**Interfaces:**
- Consumes: Task 4's `DownloadQueue` and app methods.
- Produces: `pub fn toggle_downloads_panel(&mut self, cx)` and
  `pub fn clear_finished_downloads(&mut self, cx)` (background
  `clear_finished_downloads`, revert on error).
- **Debug selectors:**
  - `downloads-toolbar-btn`, `downloads-badge`, `downloads-failure-dot`,
    `downloads-progress-ring`
  - `downloads-panel`, `downloads-backdrop`
  - `download-row-<id>`, `download-remove-<id>`, `download-retry-<id>`,
    `download-resume-<id>`, `download-again-<id>`
  - `downloads-resume-all`, `downloads-clear-finished`, `downloads-show-album`

**Rendering rules:**
- The badge text is `active_download_count()` and is hidden at 0.
- The ring is combined bytes over running entries with a known
  `total_bytes`, hidden if there are none. Draw it with a GPUI `canvas` path
  arc. If 0.2.2 has no stroked arc, draw a 2 px bar under the icon instead and
  say so in the commit body.
- Section order: Downloading, Waiting, Paused, Failed, Done (Duplicate rows
  sit in Done with "Already in your library" and Download Again).
- Status lines:
  - "142 of 380 MB · 2 min left" (from bytes and speed);
  - "Getting ready…" before any progress;
  - "Finishing…" for `Finalizing`;
  - "Next up" on the first Waiting row.
- Done rows show "Added to library", or "Removed from library" when
  `!in_library`.
- The `highlight` row gets the selection fill and the status "Already in your
  downloads". The highlight clears when the panel closes.
- "Show Downloaded" selects the Downloaded album and closes the panel.
- Rows truncate. The body scrolls with a max height of about 60% of the
  window.
- The card is about 320 px wide, anchored under the icon.
- The tooltip is "Downloads (⌥⌘L)".

- [ ] **Step 1: Write the failing tests:**
  - `toolbar_icon_hidden_without_downloads`
  - `badge_counts_active_rows`
  - `clicking_icon_toggles_panel`
  - `shortcut_toggles_panel` (`"alt-secondary-l"`)
  - `palette_toggles_panel`
  - `remove_button_cancels_running_row`
  - `keyboard_arrows_enter_backspace_escape` (Retry via ⏎ on a Failed row; ⌫
    removes; Esc closes)
  - `panel_absorbs_grid_shortcuts` (⌫ with the panel open doesn't trash the
    selected grid item)
  - `opening_panel_clears_failure_dot`
  - `clear_finished_keeps_active_rows`
  - `dropdown_truncates_and_scrolls`: 40 rows with 300-character titles. The
    panel bounds sit inside the window bounds, and the first row's width is at
    most the panel's width.
  - Click tests use `debug_bounds` centers.
- [ ] **Step 2: Run** `cargo test -p lumina-ui-gpui downloads`. Expected: the
  new tests FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the same command.** Expected: PASS. Also run
  `cargo test -p lumina-ui-gpui menus` and `cargo test -p lumina-ui-gpui palette`.
- [ ] **Step 5: Run the gates, then commit**
  `feat: show URL downloads in a toolbar dropdown`.

### Task 6: Pause on quit and on library switch, resume on reopen

**Files:**
- Create: `crates/lumina-ui-gpui/src/views/downloads_confirm.rs` (a small
  modal: occluding card plus backdrop)
- Modify: `crates/lumina-ui-gpui/src/app/render/mod.rs:417` (the `QuitApp`
  handler) and the render of the modal
- Modify: `crates/lumina-ui-gpui/src/app/library.rs:525` (`open_library`
  guard) and `open_library_inner` (load the queue)
- Modify: `crates/lumina-ui-gpui/src/app/downloads/mod.rs`
- Modify: `crates/lumina-ui-gpui/src/app/keymap/modals.rs` (the confirm keys)
- Test: `crates/lumina-ui-gpui/src/tests/downloads.rs` (extend)

**Interfaces:**
- Consumes: Task 3 `recover_downloads` and `list_downloads`; Task 4's
  scheduler.
- Produces:
  ```rust
  pub(crate) enum DownloadsConfirm { Quit, OpenLibrary(PathBuf) }
  // field on DownloadQueue: pub confirm: Option<DownloadsConfirm>
  impl LuminaApp {
      pub(crate) fn request_quit(&mut self, cx: &mut Context<Self>);          // QuitApp handler
      pub(crate) fn confirm_downloads_prompt(&mut self, cx: &mut Context<Self>); // Quit / Open
      pub(crate) fn dismiss_downloads_prompt(&mut self, cx: &mut Context<Self>);
      pub(crate) fn load_downloads(&mut self, cx: &mut Context<Self>);        // background recover + list
      pub fn resume_download(&mut self, row_id: i64, cx: &mut Context<Self>); // requeue_download, then pump
      pub fn resume_all_downloads(&mut self, cx: &mut Context<Self>);
  }
  ```
- **Debug selectors:** `downloads-confirm-card`, `downloads-confirm-backdrop`,
  `downloads-confirm-ok`, `downloads-confirm-cancel`.

**Rules:**
- **Quitting or switching library with `running_or_waiting_count() > 0`:** set
  `confirm`.
  - Copy: "1 download is still running. Quit anyway?" or "N downloads are
    still running. Quit anyway?".
  - The library switch reads "… Open another library anyway?".
  - Buttons: Quit or Open, and Cancel. ⏎ confirms, Esc and the backdrop
    cancel.
  - With no active downloads, both act at once, as today.
- **On confirm:** set every cancel flag, clear `entries` and bump
  `generation`, then `cx.quit()` or `open_library_inner(path)`. Nothing is
  written. Rows stay Running or Waiting on disk, and `recover_downloads` turns
  them into Paused at the next load. Staging is cleaned by Task 1 at the next
  open.
- **`load_downloads`** runs in `open_library_inner` after the engine opens: a
  background `recover_downloads()` then `list_downloads()`, replacing
  `entries`. It never calls `pump_downloads` (no network at launch).
- **The welcome path** (`open_library_inner` at startup) never shows the
  confirm.

- [ ] **Step 1: Write the failing tests:**
  - `quit_with_running_downloads_asks_first`: dispatch `QuitApp`. `confirm`
    is `Quit` and the app hasn't quit. Use an `on_quit` hook or an observable
    flag the test platform exposes; if neither exists, assert through a
    `#[cfg(test)] pub quit_requested: bool` set right before `cx.quit()`.
  - `quit_without_downloads_quits_at_once`
  - `confirm_escape_and_backdrop_cancel`
  - `paused_rows_load_without_starting`: write Running and Waiting rows into
    the catalog and open the library. Both show as Paused, and the fake job
    recorded no request.
  - `resume_all_starts_up_to_three`
  - `switching_library_drops_old_jobs`: run a job, confirm opening library B,
    then finish the old job's outcome channel. B's `list_downloads()` is
    empty and `entries` is empty.
- [ ] **Step 2: Run** `cargo test -p lumina-ui-gpui downloads`. Expected: the
  new tests FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Run the gates, then commit**
  `feat: pause URL downloads on quit and resume them on request`.

### Task 7: Downloaded album in the sidebar, Show in Library, docs

**Files:**
- Modify: `crates/lumina-ui-gpui/src/views/sidebar/sections.rs`: add a
  "Downloaded" nav item after Videos, shown only when its count is above 0,
  like Missing at l.148
- Modify: `crates/lumina-ui-gpui/src/app/downloads/mod.rs`: add
  `show_download_in_library`
- Modify: `crates/lumina-ui-gpui/src/app/command_palette.rs`: if the palette
  lists system albums by hand (see l.519 for Videos), add Downloaded the same
  way
- Modify: `docs/network-import.md`: describe the queue, the limit of 3, the
  quit behavior and the Downloaded album
- Modify: `docs/superpowers/specs/2026-11-03-download-queue-design.md`: set
  Status to "implemented" and note any deviation (for example a bar instead
  of a ring)
- Test: `crates/lumina-ui-gpui/src/tests/downloads.rs` (extend)

**Interfaces:**
- Consumes: Task 2 `SystemAlbum::Downloaded`; Task 4 rows; `select_album`
  (`app/library.rs:821`).
- Produces:
  `pub fn show_download_in_library(&mut self, media_id: MediaId, cx: &mut Context<Self>)`.
  It calls `select_album(AlbumSelection::System(SystemAlbum::Downloaded))`,
  then selects the item with `media_id` once the album's first page has
  loaded.
  - First check whether the session-restore path (`RestoredSession`,
    `app/state.rs:341`) already selects by id after a load, and reuse it if
    so.
  - Otherwise add `pending_select: Option<MediaId>` and apply it where the
    first page lands.
  - It also closes the panel and the dialog.
  - It is used by Done row clicks, ⏎ on a Done row, and ⏎ / Show in Library
    in `AlreadyDownloaded`.

- [ ] **Step 1: Write the failing tests:**
  - `downloaded_album_appears_after_a_download`: seed one item via
    `insert_with_source`, refresh the counts, and the sidebar shows
    "Downloaded" with a count of 1. A seed with only a sidecar source shows no
    row.
  - `done_row_click_selects_item_in_downloaded_album`: the selection is the
    Downloaded album and `selected_item.id` equals the `media_id`.
  - `already_downloaded_enter_shows_in_library`
- [ ] **Step 2: Run** `cargo test -p lumina-ui-gpui downloaded`. Expected:
  FAIL.
- [ ] **Step 3: Implement, and update the two docs.**
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Run the gates, then commit**
  `feat: add the Downloaded album and Show in Library for downloads`.

### Task 8: Full verification

- [ ] **Step 1: Run all gates**, plus `make ui-smoke`. Report a SKIP with its
  reason. A skip is not a pass.
- [ ] **Step 2: Live check in `make dev`:**
  - queue 4 URLs, mixing direct MP4, YouTube and an invalid one;
  - confirm 3 run and 1 waits, ✕ moves the queue up, and the failure shows a
    reason and a red dot;
  - press ⌘Q, Quit, reopen: the rows are Paused, then Resume all;
  - check the Downloaded album count.

  Note anything that couldn't be run. If network access is unavailable, say
  so; don't mark it done.
- [ ] **Step 3: `git status` is clean.** Don't commit `.superpowers/` (ask
  the user whether to add it to `.gitignore`).
