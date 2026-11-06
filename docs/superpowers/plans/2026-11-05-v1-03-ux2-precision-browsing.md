# UX-2 Precision Browsing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Zoom that stays under the pointer everywhere (viewer and grid, wheel, keyboard and trackpad pinch) with Fit / Fill / 1:1 presets; a video scrubber that shows real frames while hovering and dragging; and a search field whose results always match what was typed, with quick rating/color filters, saved searches and a readable explanation of the query.

**Architecture:** Pinch arrives from GPUI as `PinchEvent` and goes through the zoom math that already exists (`ViewerState::zoom_at`, `GridZoomAnchor`). Scrub previews are a small strip of frames that AVFoundation decodes in one batch call, made the first time the pointer hovers the scrubber, run on the GPUI background executor (`cx.background_executor().spawn`, not the engine's decode workers), cancellable, and kept in memory for the open video only. Search changes stay inside `StructuredQuery` (parse → explain → SQL). Quick filters become query tokens instead of a separate dimming state. A saved search is a smart album with a single new `Query` rule.

**Tech Stack:** Rust 2021, GPUI Kit 0.9 (`gpui-pre` 0.3.11), rusqlite + FTS5, AVFoundation (`AVAssetImageGenerator`) via the existing `video_frame_macos.m`. No new crates.

**Depends on / affects other plans:**
- **Plan 5 (organize), Decision 13 and its Open question 5:** viewer zoom presets move from `0/1/2/4` to `⌘0/⌘1/⌘2/⌘4` so plain digits can rate everywhere. This plan lands first (0.4 vs 0.6) and already touches every preset surface (keymap, HUD tooltips, shortcut sheet, palette), so **Task 2 makes the move** and gives Fill `⌘3`. Plan 5 then only binds plain digits to ratings; its test `cmd_digit_zoom_presets_in_viewer` is written here. If the owner rejects the move (plan 5 Open question 5), Task 2 keeps `0/1/2/4` and Fill stays palette/navigator only. Decide before Task 2 starts.
- **Plan 5 Task "flags":** it adds `GridFilter::Picks`/`Rejected` chips and reuses "the existing filter-dim opacity". Task 9 here deletes `GridFilter` and the dimming. Plan 5 must instead add `is:pick` / `is:rejected` query tokens (through Task 6's parser and Task 9's `QuickFilter`) and define its own reject opacity.
- **Plan 4 (metadata) Task 2:** tag filters become hierarchical (`tag:places` matches `places/europe`). Task 6's `-tag:x` must use the same tag predicate as `tag:x`, whichever plan lands first.

## Global Constraints

- No new dependencies. No `SCHEMA_VERSION` bump planned (14 stays). The only possible schema change is a `color_label` partial index, and only if Task 7's bench shows it is needed (see Decisions D9). If it is needed: next free version (14 -> 15 unless plan 4/5 bumped first, see the index), `CREATE INDEX IF NOT EXISTS`.
- Release perf gates stay as they are: structured query < 5 ms, smart album < 8 ms, timeline seek < 1 ms (`crates/lumina-core/tests/benchmark_test.rs:296-349`; debug budgets 25/40/5 ms). Any new filter or rule gets an `EXPLAIN QUERY PLAN` assertion in `core_tests.rs` (helper `Catalog::explain_query_plan`, `catalog/mod.rs:243`).
- No catalog lock and no AVFoundation call on the UI thread. Strip decode runs on `cx.background_executor().spawn(...)` (GPUI 0.3.11 has no `Context::background_spawn`; only the test app does), at most one strip request is in flight, and the result is dropped if the open video changed in the meantime.
- Video zoom stays disabled (`viewer_zoom_disabled_for_video`, `app/viewer_actions.rs:157`).
- UI copy names no tools (AVFoundation, FTS, SQL) and no grammar internals beyond the tokens the user typed.
- In modules that glob-import `gpui::*`, tests use `#[core::prelude::v1::test]`.
- New interactive controls get a `debug_selector`, an `id`, a tooltip and an AccessKit `.role(...)` + `.aria_label(...)` (`gpui-pre` `elements/div.rs:1357,1379`). Do not claim VoiceOver support without a manual check.
- New user-facing strings go through `src/i18n.rs` keys.
- File size (AGENTS §5.1): `app/library.rs` is 1546 lines, `lumina-core/src/search.rs` 631, `app/video_actions.rs` 624, `views/viewer/transport.rs` 606, `app/command_palette.rs` 706. New search code goes into new modules (`app/search.rs`, `lumina-core/src/search/`); scrub-strip code into `app/scrub_preview.rs` and `views/viewer/scrub_preview.rs`; palette changes stay to entries and `command_applies`/dispatch arms.
- Images leaving the screen (strip frames on close/media change) go through `LuminaApp::retire_image` (`app/state.rs:314`, drained with `cx.drop_image`) so their atlas textures are freed.
- Do not touch `crates/lumina-core/src/net_import/*` or `docs/network-import.md`. They have uncommitted changes from another session.
- Stage explicit paths. Never `git stash`.

## Review Focus

1. **Pinch drifts the picture away from the fingers.** The point under the pinch centre must stay put, the same as with the wheel. Test: `viewer_pinch_keeps_content_under_pinch_center` (Task 1).
2. **Pinch on a video, or in the grid, changes the wrong thing.** A video must not zoom. In the grid, a pinch must step one density level per threshold and must not scroll. Tests: `viewer_pinch_ignored_for_video` (Task 1), `grid_pinch_steps_one_level_per_threshold_and_anchors` (Task 3).
3. **Keyboard grid zoom loses the selected item.** After ⌘= or ⌘- the selected card must still be visible. Test: `keyboard_grid_zoom_keeps_selection_visible` (Task 3).
4. **The digit-key contract changes.** Presets on `⌘0/⌘1/⌘2/⌘3/⌘4`; plain digits no longer zoom (plan 5 gives them to ratings). Tests: `cmd_digit_zoom_presets_in_viewer`, `plain_digits_do_not_zoom_viewer` (Task 2).
5. **A stale strip shows frames from another video.** Hover video A, switch to B before A's strip arrives: B must never show A's frames, and A's decode is cancelled. Tests: `stale_scrub_strip_is_discarded_after_media_change`, `media_change_cancels_inflight_strip` (Task 5), `strip_cancel_flag_stops_batch_early` (Task 4).
6. **The strip blocks the UI or starves thumbnails.** It must run off the UI thread and outside the engine decode pool, one request at a time, and give up on undecodable files without retrying. Tests: `scrub_strip_requested_once_per_video_on_first_hover`, `undecodable_video_stays_time_only_and_is_not_retried` (Task 5), plus the existing `tests/ui_thread.rs` suite. Manual Activity Monitor check (see Task 5).
7. **`-beach` returns beach photos.** Today it does (see Current state). Test: `minus_term_excludes_fts_matches` (Task 6).
8. **A misspelled filter silently shows everything.** `is:fovorite` and `color:pink` must be reported, not ignored. Tests: `unknown_filter_values_are_reported` (Task 6), `explanation_lists_not_understood_tokens` (Task 8).
9. **Typed text reaches FTS5 as syntax.** Quotes, `OR`/`NOT`/`NEAR`, `*`, `^`, `:` and lone `-` must be literal text and never raise an SQL error. Test: `fts_syntax_in_query_text_is_literal` (Task 6).
10. **Filters contradict the query text.** No hidden dimming state may survive Task 9. Search results must not sit under a highlighted album they ignore, whether the query was typed or set by a command. Tests: `palette_filter_toggles_query_token`, `typing_a_query_moves_sidebar_to_library` (Task 9).
11. **A saved search drifts from the search it saved.** The same text gives the same rows as a search and as a smart album, and the perf gate holds. Tests: `query_rule_album_matches_search_rows` and `query_rule_plan_uses_fts_index` (Task 10), bench `benchmark_scale_and_gates`.

---

## Current state (verified 2026-11-05)

Several "current" claims in ROADMAP.md §7–§8 and ux_audit.md §4/§5/§7 are stale.

**Viewer zoom: mostly done already.**
- Cursor-anchored, multiplicative wheel and trackpad-scroll zoom: `views/viewer/viewport.rs:18` (`scroll_zoom_delta`), `:170-189` (`on_scroll` → `zoom_viewer_at`), `views/viewer_state.rs:187` (`zoom_at`). The anchor is the painted canvas centre (`viewer_actions.rs:214-219`, falling back to the window centre before first paint), so anchoring is exact in the fullscreen viewer and approximate in Split layouts (documented at `viewer_actions.rs:177-186`).
- Presets Fit / Actual / 2x / 4x on plain `0/1/2/4` (no modifier): `viewer_state.rs:97-134`, `app/keymap/viewer.rs:48-60`. `Z` and double-click on the media toggle Fit/1:1 (`viewer_state.rs:236`, `viewport.rs:197-204`). The shortcut sheet lists "0 / 1 / 2 / 4" (`app/render/mod.rs:153`); the HUD Fit button tooltip says "Fit to window (0)" (`stage_controls.rs:30`); the keyboard test `test_viewer_zoom_presets_hud_label_and_shortcuts` (`tests/viewer_and_video.rs:564`) presses plain digits.
- No `⌘digit` is bound anywhere today (`app/keymap/mod.rs`, `grid.rs`, `viewer.rs`; `menus.rs` has no key equivalents), so `⌘0`–`⌘4` are free in the viewer.
- There is a zoom HUD with a percentage readout, a navigator and a Fit button (`views/viewer/stage_controls.rs:14-140`, selectors `viewer-zoom-fit`, `viewer-navigator`, `viewer-zoom-label`). It renders **only while zoomed** (`zoom_scale > ZOOM_MIN_SCALE`, `:16-18`). Max zoom is 16x (`viewer_state.rs:93`).
- ux_audit §4 says both "cursor-anchored" and "no cursor anchoring / no presets". The second claim is stale.
- **Missing:** pinch, a Fill preset, and any palette or menu path to the zoom presets. The palette has only grid zoom (`app/command_palette.rs:310-321`); `menus.rs` has no View > Zoom.
- **Stale doc comment:** `app/viewer_actions.rs:188` says the clamp is "1.0..5.0". It is 1..16.
- **Limitation:** stills decode at 2048 px (`app/viewer_actions.rs:47`), so "100%" of a larger photo is an enlarged decode.

**GPUI pinch (checked in `~/.cargo/registry/src/*/gpui-pre-0.3.11`).**
- `PinchEvent { position, delta, modifiers, phase }` exists (`gpui-pre-0.3.11/src/interactive.rs:591-605`); `delta` is the per-event magnification change (0.1 = +10%). `on_pinch` / `capture_pinch` on div (`elements/div.rs:411,441`); `on_pinch` fires in the bubble phase only when the element's hitbox is hovered.
- On macOS it is fed from `NSEventType::Magnify` (`gpui-pre-macos-0.3.11/src/events.rs:243-261`, `window.rs:164` `magnifyWithEvent:`). Phase mapping: `MayBegin`/`Began` → `Started`, `Ended` → `Ended`, everything else (including a cancelled gesture) → `Moved`. **`TouchPhase::Cancelled` never arrives from a trackpad.** Smart zoom (two-finger double-tap) is not delivered.
- Dispatch sets `window.mouse_position = pinch.position` (`gpui-pre-0.3.11/src/window.rs:5900-5904`), so a synthetic pinch hit-tests like a scroll.
- Pinch recognition for *touch screens* is not implemented (`gestures.rs:527`). That does not affect trackpads.
- Tests inject the event with `VisualTestContext::simulate_event` (`gpui-pre-0.3.11/src/app/test_context.rs:1068`; call `draw`/`render_ui` first). (`app/visual_test_context.rs:326` is the real-window `VisualTestAppContext`, not what Lumina's tests use.)
- Lumina uses `on_pinch` nowhere (grep).

**Grid zoom.**
- ⌘-wheel is pointer-anchored (`app/panels.rs:171-280` `zoom_grid_with_wheel`, `views/grid.rs` `GridZoomAnchor`/`GridZoomGeometry`; tests `ui_automation.rs:2070,2118,2175`).
- ⌘-wheel steps one column per scroll event across 2–8 (`panels.rs:214-218`), with no accumulation: any event above `GRID_ZOOM_WHEEL_EPSILON = 0.01` px (`panels.rs:11`) is a step, so one trackpad ⌘-scroll burst runs through several densities. ⌘= / ⌘- and the palette instead step the 5-level slider ladder `SIZE_COLUMNS = [8,6,4,3,2]` (`panels.rs:150-169`, `views/search_bar.rs:211`). So the wheel can land on 5 or 7 columns, which the slider cannot show (`size_level` rounds them).
- Keyboard grid zoom neither anchors nor reveals the selection (`step_grid_size` has no scroll adjustment), even though `reveal_grid_selection` exists (`app/navigation.rs:503`).
- `grid_columns` is a flat `LuminaApp` field; `GridState` (`views/grid.rs:316`) is the per-library engine/cache handle, recreated on library open, not a UI-gesture struct.
- No pinch, no animated reflow.

**Video scrub.**
- Drag-scrub is shipped: press/drag/release, a window-wide drag tracker, seeks coalesced to 50 ms, and a commit on release (`views/viewer/transport.rs:15-290`, `app/video_actions.rs:19,438-487`). While dragging, the *main* frame follows, because each coalesced seek arms the paused frame refresh (`video_actions.rs:391-401`).
- Hover shows a time badge only. This is deliberate (`transport.rs:123-125`), and the test `tests/video_scrub.rs:195` `scrubber_feedback_is_a_time_badge_not_a_synthetic_preview` locks it in.
- Scrubbing does not pause playback (`begin_video_scrub` never touches `is_playing`).
- The video state struct is `VideoPlaybackState` (`app/state.rs:72-121`, field `self.video`); there is no `VideoState`. `stop_video_player` (`video_actions.rs:217-252`) runs on close, media change (via `load_video_player`, `:168`), PiP close and shutdown, and clears scrub fields at `:241-251`.
- Resume (`video_resume`) and HUD auto-hide (`hud_idle`, `app/state.rs:345`; `viewer_actions.rs:381`) are shipped. Speed and mute are session-only (`state.rs:580-582`: speed 1.0, volume 0.85); there is no volume control in the UI and no config field for either.
- `lumina_video_frame` (`lumina-core/src/video_frame_macos.m:33`) takes no time argument. It requests [1 s, 0 s] in one `AVAssetImageGenerator` batch, with a 10 s timeout (`decoder.rs:30,306-308`). The completion block writes only into a retained `LuminaFrameGrab` object, never into caller memory, because callbacks can fire after a timed-out call has returned. `video_frame` is a private module (`lib.rs:13`).
- The player bridge (`player/backend.rs:20-43`) has a single `AVPlayer` + `AVPlayerItemVideoOutput`. Using it for hover previews would move the playhead. There is **no stub player**: on macOS, tests get the real `AvFoundationPlayer` (`player/mod.rs:11-19`).
- `bridge::frame_to_gpui_image` (`bridge.rs:8`) turns a `DecodedFrame` into an `Arc<RenderImage>`.

**Search and filters.**
- Recent search history is shipped: 10 entries, persisted in `config.recent_searches` (`config.rs:363`, `app/library.rs:29,163-180`), and shown in the suggestion dropdown together with grammar tokens and cached tags (`library.rs:48-161`, `views/search_bar.rs:118-180`).
- There is no way to clear the history. Recent rows look the same as operator rows.
- `save_config` (`app/state.rs:630-671`) returns `()`, is a no-op under `cfg(test)`, and on failure only sets `error_message`.
- The filter bar was removed in 839574a ("redundant filter bar"). `GridFilter` still exists (`search_bar.rs:186-208`). It *dims* non-matching cards (`app/render/grid.rs:206-211,364-375`, selector `media-card-dimmed-{id}`), can be set only from the palette (`command_palette.rs:322-351`, applied at `:670`), and has no on-screen indicator. A user who picks "Filter: Favorites" sees a dimmed grid with no visible reason. Locked in by `tests/browser.rs:111` `filter_chips_select_a_filter_that_dims_rather_than_hides`.
- **Search ignores the sidebar scope.** A non-empty query replaces the album, tag or year view (`app/active_view.rs:46-47,74-75`). `set_search_query` (`library.rs:993-999`) clears `active_year` but leaves `selection` highlighted. **Typing does not go through `set_search_query` at all:** keystrokes edit `search_query` in place (`app/text_input.rs:532`) and `text_edited(TextTarget::Search)` (`text_input.rs:568-571`) only resets suggestions and clears the selection. So "Favorites" stays highlighted while library-wide results show.
- Grammar in `lumina-core/src/search.rs`:
  - Unknown `is:` / `has:` / `color:` values are dropped silently (`:86-93`, `:94-100`, `:104-119`), so `is:favourite` matches everything. Unparseable `year:`/`month:`/`date:`/`iso:`/`f:`/`rating:` values are dropped silently too (`:42-56`, `:77-85`, `:101-103`, `parse_num_filter` `:502-523`).
  - `is:fav` already exists (`:87`); `is:favourite` does not.
  - There is no negation. `-beach` becomes the FTS phrase `"-beach"*` (`build_fts5_expression`, `:548-556`), and the tokenizer drops the `-`, so it *matches* beach.
  - FTS escaping today: each term has `"` and `*` stripped, then is wrapped as `"term"*` and terms are joined with spaces (implicit AND). Inside a string FTS5 operators (`OR`, `NOT`, `NEAR`, `col:`) are literal, so this is injection-safe; no test pins it.
  - `has_tag = Some(false)` and `has_location = Some(false)` compile (`:235-251`) but no token reaches them.
  - `is:photo` means non-RAW images only (`media_type = 'image'`). This is not documented.
  - With FTS terms the query is `FROM media_search_fts JOIN media_items m ON rowid = m.id` (`:315-335`, count `:368-378`); smart albums always select `FROM media_items m` and append `m.is_trashed = 0 AND m.is_hidden = 0` (`smart_albums/rules.rs:102-128,139-170`).
- Saved searches: none. Smart albums have no free-text rule (`smart_albums/rules.rs:14-38`). The builder offers `FIELDS` (`app/smart_draft.rs:14-28`) and still shows legacy fields; `open_draft` is private in `app/album_builder_actions.rs:55`.
- Indexes: there is no `color_label` index (`catalog/schema.rs:67-76`). Rating has `idx_media_rating(rating DESC, id DESC)`.
- The bench gate covers only `camera:Sony is:favorite iso:>800` and the Favorites system album. The bench catalog (`benchmark_test.rs:168-260`) seeds favorites (`i % 7`) but no ratings, colour labels or distinctive file-name words, so rating/colour/FTS-exclusion queries would bench against empty matches.
- No `CHANGELOG` file exists in the repo.

## Decisions

- **D1 Pinch reuses the wheel path.** Viewer: `delta_scale = zoom_scale * pinch.delta`, sent to `zoom_viewer_at`. *Rejected:* a separate gesture model with inertia. It would add complexity and nobody has asked for it.
- **D2 Two-finger scroll keeps zooming in the viewer.** No behaviour change; pinch is added alongside it. *Rejected:* switching scroll to pan when zoomed (the Photos.app convention). That is a UX change the owner should decide (Open question 2).
- **D3 Fill preset; presets on ⌘digits.** `Fill = max(canvas.w / fitted.w, canvas.h / fitted.h)`. Presets move to `⌘0` Fit, `⌘1` 100%, `⌘2` 200%, `⌘3` Fill, `⌘4` 400% (plan 5 Decision 13 frees plain digits for ratings). They are also navigator buttons (Fit · Fill · 1:1, visible while zoomed like the rest of the HUD) and palette commands. `F` stays Favorite. *Fallback* if the owner keeps plain digits for zoom: `0/1/2/4` unchanged and Fill is palette/navigator only (no `3`: plan 5 would need it as a rating).
- **D4 One grid density ladder.** ⌘-wheel and pinch walk `SIZE_COLUMNS` like the keyboard and slider do; pixel (trackpad) ⌘-scroll accumulates to a threshold per step like pinch, a line (mouse-wheel notch) is one step. *Rejected:* keeping the 7-step wheel. The slider cannot show 5 or 7 columns.
- **D5 Animated grid reflow is out of scope.** GPUI's uniform list has no layout transition. Faking one costs a second render pass for little gain.
- **D6 Scrub previews: a lazy, cancellable batch strip.** `count = clamp(ceil(duration_s), 2, 48)` frames (at most one per second of video), long edge 256 px, decoded in one `AVAssetImageGenerator` call with tolerance ± half the spacing (so it snaps to keyframes and stays fast). The strip is built on the first scrubber hover (or press) for the open video, only when the player loaded it, held in `VideoPlaybackState`, and dropped (through `retire_image`) on close or media change. Memory: 48 × 256×144 × 4 B ≈ 7 MB for 16:9, ≤ 12.6 MB worst case (square), plus atlas textures for frames actually painted.
  - Cancellation: the request owns an `Arc<AtomicBool>`; the stop path sets it and the batch call polls it, cancels generation and returns. Leaving the scrubber does **not** cancel: the strip is per video and reused on the next hover.
  - Failure (unsupported codec such as VP9-in-MP4, unreadable file, timeout with zero frames): `log::warn!`, stay time-only, and do not retry for that item until it is reopened.
  - *Rejected:* a per-hover exact decode (opens an asset per move, queues work, needs debounce and an LRU).
  - *Rejected:* a second `AVPlayer` (heavy, owns audio).
  - *Rejected:* the engine decode pool (it would compete with grid thumbnails).
  - *Rejected:* a disk cache (YAGNI).
  - Known ceiling: on long videos the hover frame is the nearest strip frame. A 2-hour video has 2.5-minute spacing. While dragging, the main frame shows the exact seek.
- **D7 Pause while scrubbing.** If the video was playing, it pauses on press and resumes on release. This prevents audio thrash and matches the QuickTime behaviour the audit asks for.
- **D8 Quick filters are query tokens.** Palette "Filter" commands toggle `is:favorite`, `rating:>=N`, `color:<name>`, `is:unrated` and the like in the search text. `GridFilter` and its dimming are deleted.
  - *Rejected:* re-adding the chip bar (the owner removed it 3 days ago).
  - *Rejected:* keeping the dimming with an indicator (it would be a second filter system that can contradict the query).
  - Open question 1 offers a single toolbar Filter menu as an addition.
- **D9 Color index only if measured.** Task 7 adds bench queries for `color:red` and `rating:>=4` against a catalog seeded with realistic label/rating spreads. A partial index `idx_media_color(color_label, timestamp_epoch_ms DESC, id DESC) WHERE is_trashed = 0 AND is_hidden = 0` (with the migration and the version bump) is added only if the release gate fails.
- **D10 A saved search is a smart album with `RuleField::Query`.** The value is the raw query text. It compiles through `StructuredQuery` with the FTS match as `m.id IN (SELECT rowid FROM media_search_fts WHERE media_search_fts MATCH ?)`. The sidebar, counts, rename, edit, delete confirmation and ordering all come for free. Rules are JSON in `smart_albums.rules_json`, so there is no schema change. Recent-search history stays in config (per machine, already shipped).
  - *Rejected:* a `saved_searches` list in config.json. It would be per-machine, lost when the library moves, and would need new sidebar UI.
  - *Rejected:* converting a query into existing rules. FTS terms have no rule, and two compilers would drift apart.
- **D11 Search scope tells the truth.** When a query becomes non-empty, by typing, a suggestion, a quick filter or any `set_search_query` caller, `selection` becomes `AllLibrary`, as `active_year` already does. Back restores the album through the existing navigation history.
  - *Rejected:* scoping search to the selected album, which means joining smart-album SQL with structured SQL. The owner may want this (Open question 3).
- **D12 Value autocomplete (cameras, lenses, cities) is deferred.** It needs a cached DISTINCT map refreshed on scans. The explanation and the token vocabulary give discoverability first.
- **D13 Unknown filter values are reported, not searched.** They land in `StructuredQuery::unrecognized`, add no clause, and are shown as "Not understood: …" in the explanation; a smart-album `Query` rule refuses them. Unknown *prefixes* (`foo:bar`, `12:30`, plan 5's malformed `within:`) stay free text as today.

---

## Part A: Zoom

### Task 1: Trackpad pinch in the viewer (S)

**Files:**
- Modify: `crates/lumina-ui-gpui/src/views/viewer/viewport.rs` (add `on_pinch` on `media-canvas-fit` next to `on_scroll`)
- Modify: `crates/lumina-ui-gpui/src/app/viewer_actions.rs` (fix the stale clamp doc at `:188`; mention pinch in the `zoom_viewer_at` doc)
- Test: `crates/lumina-ui-gpui/src/tests/viewer_and_video.rs`, `views/viewer/viewport.rs` unit tests

**Interfaces:**
- `fn pinch_zoom_delta(pinch_delta: f32, scale: f32) -> f32` in `viewport.rs`. It returns `scale * pinch_delta.clamp(-0.5, 0.5)`, and 0 for non-finite input.
- The listener calls `this.zoom_viewer_at(pinch_zoom_delta(e.delta, scale), (pos.x, pos.y), viewport, cx)` and `cx.stop_propagation()`. Phase is ignored (each event is an increment).

Steps:
- [ ] Failing tests:
  - `pinch_zoom_delta_is_multiplicative_and_finite` (unit: same `delta` at 1x and 8x gives proportional changes; NaN/inf give 0).
  - `viewer_pinch_keeps_content_under_pinch_center`: fullscreen viewer (where the anchor is exact), `render_ui`, then `vcx.simulate_event(PinchEvent{position, delta: 0.2, phase: Moved, ..})` at an off-centre point inside `viewer-media` bounds. Check that the scale grows by 20% and that the content point under the pinch maps to the same window point (same formula as `test_viewer_cursor_anchored_zoom_action`, `tests/viewer_and_video.rs:773`).
  - `viewer_pinch_ignored_for_video` (scale and pan unchanged).
  - `pinch_out_to_fit_recenters` (pinch in, pan, pinch out past Fit: scale 1.0, pan (0,0)).
- [ ] Implement, then run `cargo test -p lumina-ui-gpui pinch`.
- [ ] Manual: pinch on a real trackpad (cannot be automated; record the result in the commit body).
- Keyboard path: unchanged (`+ -`, presets per Task 2, `Z`). Docs: README viewer bullet ("pinch to zoom").

### Task 2: Presets on ⌘digits, Fill preset, preset buttons, palette zoom commands (M)

Blocked on the owner's answer to plan 5 Open question 5 (see "Depends on"). The steps below assume the move; the fallback is noted inline.

**Files:**
- Modify: `views/viewer_state.rs` (`ViewerZoomPreset::Fill`; `ViewerCanvas` already carries `size` and `fitted`, `:43-51`; update the "0/1/2/4" doc at `:97`)
- Modify: `app/keymap/viewer.rs:48-60` (presets require `cmd_or_ctrl`: `⌘0` Fit, `⌘1` Actual, `⌘2` 2x, `⌘3` Fill, `⌘4` 4x; plain digits fall through and do nothing in this plan) and the doc comment at `:10-11`
- Modify: `app/viewer_actions.rs:437` doc
- Modify: `views/viewer/stage_controls.rs` (three preset buttons in the navigator; Fit tooltip "Fit to window (⌘0)" at `:30`; doc at `:11-13`)
- Modify: `app/command_palette.rs` (`CommandId::ViewerZoom(ViewerZoomPreset)` with shortcut column `⌘0`…`⌘4`; `command_applies` is true only while a still is open in the viewer)
- Modify: `i18n.rs` (`command.zoom_fit` "Zoom to Fit", `command.zoom_fill` "Zoom to Fill", `command.zoom_actual` "Actual Size", `command.zoom_200` "Zoom 200%", `command.zoom_400` "Zoom 400%")
- Modify: `app/render/mod.rs:153` shortcut sheet ("⌘0 / ⌘1 / ⌘2 / ⌘4" → "Fit / 100% / 200% / 400%", plus "⌘3" → "Fill")
- Test: `viewer_state.rs` unit, `tests/viewer_and_video.rs` (update `test_viewer_zoom_presets_hud_label_and_shortcuts`, `:564`, whose keyboard half presses plain digits), `tests/palette_and_empty.rs`, `app/render/mod.rs` sheet test

**Interfaces:**
- `Fill.scale(canvas) = canvas.map_or(1.0, |c| (c.size.0 / c.fitted.0).max(c.size.1 / c.fitted.1)).clamp(ZOOM_MIN_SCALE, ZOOM_MAX_SCALE)`.
- Selectors: `viewer-zoom-preset-fit`, `viewer-zoom-preset-fill`, `viewer-zoom-preset-actual`. Each has a tooltip ("Fit (⌘0)", "Fill (⌘3)", "Actual size (⌘1)"), `Role::Button` and an `aria_label`. They live in the navigator, which renders only while zoomed; at Fit the paths to Fill are `⌘3` and the palette.
- When the media already fills the canvas, Fill equals Fit: the button stays enabled and `set_zoom` recentres.
- Fallback (owner keeps plain digits): keymap and sheet unchanged, Fill has no key, tooltips "Fit (0)", "Fill", "Actual size (1)", and the two keyboard tests below become `plain_digit_zoom_presets_unchanged`.

Steps:
- [ ] Failing tests:
  - `fill_preset_covers_canvas_on_both_axes` (wide photo in a tall canvas gives scale `size.h / fitted.h`; square-on-square gives 1.0; no canvas gives 1.0).
  - `cmd_digit_zoom_presets_in_viewer` (`⌘1`, `⌘2`, `⌘3`, `⌘4`, `⌘0` through the real keymap with `secondary-` keystrokes: each sets the expected scale and `zoom_label`).
  - `plain_digits_do_not_zoom_viewer` (`1`, `2`, `4`, `0` leave scale at Fit; this is the contract plan 5 builds on).
  - `navigator_preset_buttons_apply_presets` (zoom to 2x first so the navigator renders, then click each selector at rendered bounds, assert `zoom_label`).
  - `palette_zoom_commands_only_in_photo_viewer` (absent in the grid and for video; "Zoom to Fill" via ⌘K + typing + Enter changes the scale).
  - Shortcut sheet test: `says("⌘3") == Some("Fill")` and no row for plain "0 / 1 / 2 / 4".
- [ ] Update `test_viewer_zoom_presets_hud_label_and_shortcuts` to press `secondary-1` etc.
- [ ] Implement, test, then run `make ui-smoke` (the layout changes).
- Docs: README viewer bullet (new keys); ux_audit §4 "Current" rewritten from the verified state. Tell plan 5's owner that the key move landed.

### Task 3: One grid ladder, anchored keyboard zoom, grid pinch (M)

**Files:**
- Modify: `app/panels.rs`. Extract `fn zoom_grid_at(&mut self, step: isize, anchor_pos: Option<Point<Pixels>>, scroll_y_before: Pixels, cx)` from `zoom_grid_with_wheel`. The wheel, `step_grid_size` and pinch all call it. It walks `SIZE_COLUMNS` through `size_level`.
- Modify: `app/render/grid.rs` (add `on_pinch` on the grid surface next to the `on_scroll` at `:112`)
- Modify: `app/state.rs`: a small `GridZoomGesture { pinch_accum: f32, wheel_accum_px: f32 }` struct as one `LuminaApp` field (AGENTS §5.3: no new flat fields; `GridState` is the per-library engine handle and is the wrong home)
- Test: `tests/ui_automation.rs`, `tests/windows_and_layout.rs`

**Interfaces:**
- Keyboard ⌘= / ⌘- anchors on the selected item when there is one (then `reveal_grid_selection`). Otherwise it anchors on the item at the viewport's top-left, through `GridZoomAnchor`.
- Pinch accumulates `delta`. Each time the total passes ±`GRID_PINCH_STEP = 0.12` it steps one level and subtracts the step. The accumulator resets on `TouchPhase::Started` and `Ended` (macOS never sends `Cancelled`, see Current state).
- ⌘-scroll: `ScrollDelta::Lines` is one step per event as today; `ScrollDelta::Pixels` accumulates into `wheel_accum_px` and steps per `GRID_WHEEL_STEP_PX = 40.0`, resetting on `TouchPhase::Started`/`Ended`. Plain wheel stays scrolling.
- Config is written through `schedule_session_save` (debounced) for wheel and pinch. The keyboard path keeps `save_config`.

Steps:
- [ ] Failing tests:
  - `cmd_wheel_walks_size_ladder` (6 columns + one line of wheel-out gives 8, not 7).
  - `trackpad_cmd_scroll_burst_steps_once_per_threshold` (10 pixel events of 5 px at 4 columns: exactly one step, to 3).
  - `keyboard_grid_zoom_keeps_selection_visible` (select index 40 at 8 columns, ⌘= twice, then the selected card's bounds intersect the viewport).
  - `grid_pinch_steps_one_level_per_threshold_and_anchors` (5 events of +0.05 give exactly one level, the item under the pinch stays in the anchored row, and the scroll offset is otherwise unchanged).
  - Keep `ui_automation_plain_wheel_scrolls_and_cmd_wheel_zooms_around_pointer` (4 → 3 still holds on the ladder; if it uses pixel deltas below 40 px, raise its delta rather than the threshold, and say so in the commit).
- [ ] Implement, test, `make ui-smoke`.
- Keyboard path exists (⌘= / ⌘-, palette). Docs: shortcut sheet unchanged; README "grid density" bullet mentions pinch.

## Part B: Video scrub previews

### Task 4: Cancellable batch frame grab at given times (core, M)

**Files:**
- Modify: `crates/lumina-core/src/video_frame_macos.m` (add `lumina_video_frames`; `lumina_video_frame` is unchanged)
- Modify: `crates/lumina-core/src/video_frame.rs` (`decode_at`, `video_preview_strip`)
- Modify: `crates/lumina-core/src/lib.rs` (`pub use video_frame::video_preview_strip;` only; `lib.rs` does not grow)
- Modify: `crates/lumina-core/src/decoder.rs:30` (`NATIVE_TOOL_TIMEOUT` becomes `pub(crate)`)
- Test: unit tests in `video_frame.rs` (macOS-gated, fixture `crates/lumina-ui-gpui/tests/fixtures/clip.mp4`, 64×48 H.264)

**Interfaces:**
- C: `int lumina_video_frames(const char *path, int max_size, const double *times, int count, double tolerance_s, double timeout_s, const volatile int32_t *cancel, LuminaFrameOut *out)`, where `typedef struct { double time; int width; int height; uint8_t *data; } LuminaFrameOut;`. Returns the number of filled slots (0..=count), written to `out[0..n]` in ascending `time` (the *actual* frame time from the callback).
  - Memory safety (same rule as `LuminaFrameGrab`): the completion block writes **only** into a retained Objective-C holder sized `count`; `out` is filled from the holder after waiting, under `@synchronized`. Callbacks that fire after return touch only the holder.
  - Behaviour: one generator; `appliesPreferredTrackTransform = YES`; tolerance before and after = `tolerance_s`; one deadline for the whole batch, waited in 50 ms slices; between slices, if `*cancel != 0` or the deadline passed, call `cancelAllCGImageGeneration` and keep the slots completed so far.
  - Each `data` is freed with `lumina_video_frame_free`.
  - Shared BGRA conversion: factor the existing CGBitmapContext block (`:96-112`) into `static uint8_t *lumina_bgra(CGImageRef, int *w, int *h)`.
- Rust: `pub(crate) fn decode_at(path: &Path, times: &[f64], max_size: u32, tolerance: Duration, timeout: Duration, cancel: &AtomicBool) -> Result<Vec<(f64, DecodedFrame)>>`. It errors on non-macOS. It returns frames in ascending time order. An empty result is an `Err` ("no frames"). (`AtomicBool` is passed to C as a pointer to its `i32`-sized storage via an `AtomicI32`; pick whichever is sound and add a SAFETY comment.)
- Public: `pub fn video_preview_strip(path: &Path, duration_secs: f64, max_size: u32, cancel: &AtomicBool) -> Result<Vec<(f64, DecodedFrame)>>` in `video_frame.rs`. `count = clamp(ceil(duration), 2, 48)`; times are `(i + 0.5) * duration / count`, tolerance is `duration / count / 2`; timeout `NATIVE_TOOL_TIMEOUT` (10 s). `duration_secs` ≤ 0 or non-finite is an `Err`.

Steps:
- [ ] Failing tests:
  - `strip_returns_sorted_frames_within_size` (fixture, `max_size` 32: 1..=count frames, times ascending and inside the duration, each ≤ 32 px long edge (the fixture is 64×48, so this proves scaling), opaque, not all black).
  - `strip_of_missing_file_errors` (no panic, returns well under the timeout).
  - `strip_of_undecodable_file_errors_fast` (a temp `.mp4` of random bytes stands in for a codec AVFoundation cannot read, such as VP9-in-MP4: `Err`, under 2 s).
  - `strip_with_zero_or_nan_duration_errors`.
  - `strip_cancel_flag_stops_batch_early` (cancel set before the call: returns `Err` or fewer frames, under 200 ms).
- [ ] Implement, then run `cargo test -p lumina-core video_frame`. Check `cargo clippy` for the FFI `unsafe` blocks (each one has a SAFETY comment).

### Task 5: Hover and drag previews on the scrubber, pause while scrubbing (M)

**Files:**
- Create: `app/scrub_preview.rs` (`request_scrub_strip`, `scrub_preview_at`, completion handler, test seam), registered in `app/mod.rs`
- Modify: `app/state.rs` (add to `VideoPlaybackState`: `preview_strip: Option<ScrubStrip>`, `strip_request: Option<StripRequest>`, `resume_after_scrub: bool`)
- Modify: `app/video_actions.rs` (pause/resume in `begin_video_scrub`/`end_video_scrub`; call `request_scrub_strip` from `set_video_hover_scrub(Some)` and `begin_video_scrub`; in `stop_video_player` (`:241-251`) set the cancel flag, retire the strip's images, clear the new fields)
- Create: `views/viewer/scrub_preview.rs` (preview card above the track: the image plus the existing time badge); `views/viewer/transport.rs` only calls it and drops the "honest badge" comment at `:123-125`
- Test: `tests/video_scrub.rs`

**Interfaces:**
- `pub(crate) struct ScrubStrip { pub item: MediaId, pub frames: Vec<(f64, Arc<RenderImage>)> }`.
- `pub(crate) struct StripRequest { pub item: MediaId, pub cancel: Arc<AtomicBool>, pub failed: bool }`. Kept until the item closes, so one item gets at most one request per open.
- `fn request_scrub_strip(&mut self, cx)`. It runs only if the player loaded the item (`video.player.is_some()`), `video.duration > 0`, and `strip_request` is `None` for this item. It spawns `cx.background_executor().spawn(video_preview_strip(path, duration, 256, &cancel))`, converts with `bridge::frame_to_gpui_image` on the background thread, then on the UI thread stores the strip only if `self.video.item` is still that id **and** the request's cancel flag is unset (an A → B → A reopen gets a fresh request; the old one is cancelled and its result dropped). On error it logs `log::warn!`, sets `failed = true`, and stays time-only. There is no banner, because the preview is an enhancement.
- `fn scrub_preview_at(&self, time: f64) -> Option<Arc<RenderImage>>`: nearest frame by binary search on frame time.
- Test seams: `#[cfg(test)] fn set_scrub_strip_for_test(&mut self, strip: ScrubStrip)` and a `#[cfg(test)]` thread-local counter + result injector in place of the real spawn (tests never call AVFoundation from the UI crate).
- Selector: `video-scrub-preview`, with the image keyed by frame time. The card has no `occlude()` and no mouse listeners, so it never steals the drag.

Steps:
- [ ] Replace `scrubber_feedback_is_a_time_badge_not_a_synthetic_preview` with two tests:
  - `scrubber_without_strip_shows_time_badge_only`.
  - `hover_shows_nearest_strip_frame` (inject a 4-frame strip, hover at 60%: `video-scrub-preview` renders with the frame nearest 0.6 × duration, and the time badge is still there).
- [ ] Failing tests:
  - `drag_scrub_shows_preview_for_destination`.
  - `stale_scrub_strip_is_discarded_after_media_change` (call the completion handler with item A's strip after opening B: no strip).
  - `media_change_cancels_inflight_strip` (request for A, open B: A's cancel flag is set).
  - `scrub_strip_requested_once_per_video_on_first_hover` (counter: hover, leave, hover again gives 1; reopening the item gives 2).
  - `undecodable_video_stays_time_only_and_is_not_retried` (injected error: no preview card, time badge shown, a second hover does not request again).
  - `closing_viewer_retires_strip_images` (strip images land in `retired_images`).
  - `scrub_pauses_playing_video_and_resumes_on_release` (the fixture clip loaded in the real player, which is what tests get on macOS; start playback, press the scrubber: `is_playing` false; release: true).
  - `scrub_of_paused_video_stays_paused`.
- [ ] Implement, test, `make ui-smoke`.
- [ ] Manual (record in the commit): on a 4K/HEVC file, while playing, hover the scrubber. The first preview appears in under about 1 s; grid thumbnails keep loading; no frame drops are visible. Activity Monitor shows a short CPU/GPU spike, not a sustained one. Arrow through several videos with the pointer resting on the scrubber: CPU settles (cancelled batches stop). Hover a file the player cannot open: no error, time badge only. These need real hardware.
- Keyboard path: frame step `,` `.`, J/K/L and arrows already seek and show the frame. The preview is pointer-only by nature, and the keyboard alternative is documented in the shortcut sheet. Docs: README video bullet; ux_audit §5 "Current".

### Task 5b: Remember playback speed and mute (S)

ROADMAP §7 and the UX-2 list ask for persisted speed/volume. There is no volume control in the UI (volume is fixed at 0.85), so only speed and mute are persisted.

**Files:** `config.rs` (`video_speed: Option<f32>`, `video_muted: bool`, both `#[serde(default)]`), `app/state.rs` (initial values from config, clamped to the speed menu's values), `app/video_actions.rs` (`set_video_speed`, `toggle_video_mute` call `save_config`)
- [ ] Failing tests: `config_video_speed_and_mute_roundtrip_and_default` (config unit, temp path); `video_speed_and_mute_come_from_config` (UI test with an injected config; nothing persisted under `cfg(test)`).
- [ ] Implement, test. Docs: README video bullet.

## Part C: Search and filters

### Task 6: Grammar correctness: negation, reported unknowns, aliases, FTS safety (core, M)

Split into two commits: (a) the module move with no behaviour change, (b) the grammar changes.

**Files:**
- Move: `crates/lumina-core/src/search.rs` → `crates/lumina-core/src/search/mod.rs` (SQL) + `search/parse.rs` (tokenize, parse, value parsers). Do the move first with no behaviour change; `pub use` keeps `lumina_core::search::{StructuredQuery, tokenize_query}` paths.
- Modify: `search/mod.rs` (new clauses)
- Test: unit tests in `search/parse.rs` and `search/mod.rs`; EXPLAIN and row tests in `crates/lumina-core/tests/core_tests.rs`

**Interfaces (`StructuredQuery` additions, all `#[serde(default)]`):**
- `pub fts_exclude: Vec<String>` from bare `-word` / `-"two words"`. SQL: `m.id NOT IN (SELECT rowid FROM media_search_fts WHERE media_search_fts MATCH ?)`, with one expression: each term escaped exactly like `build_fts5_expression` (strip `"` and `*`, wrap `"term"*`), joined with ` OR `. Prefix semantics match inclusion, so `-beach` also hides "beaches" (documented in `docs/search.md`).
- `pub exclude_tags: Vec<String>` from `-tag:x`. SQL: `NOT EXISTS (SELECT 1 FROM media_tags WHERE media_id = m.id AND <tag predicate>)`, using the same predicate helper as `tag:x` (plan 4 makes it hierarchical).
- Negated booleans: `-is:favorite` sets `is_favorite = Some(false)`. `-has:location`, `-has:tag` and `-has:color` set `Some(false)`.
- New aliases:
  - `is:untagged` means `has_tag = Some(false)`.
  - `is:unrated` means `max_rating = Some(0)`.
  - `is:unlabeled` and `color:none` mean `has_color_label = Some(false)`. Today `color:none` sets `color_label = Some(0)`; switch it so there is one clause.
  - `is:favourite` means favorite (`is:fav` already exists).
  - `color:grey` and `color:gray` are not labels and stay unknown.
- `pub unrecognized: Vec<String>` (D13) collects, as typed:
  - unknown `is:`, `has:` and `color:` values;
  - unparseable `rating:`, `iso:`, `f:`, `year:`, `month:` and `date:` values (the value parsers return `Option`/`bool` so failure is visible);
  - unsupported negations such as `-is:video` or `-camera:x`.
  These tokens are *not* searched as text. Unknown *prefixes* (`foo:bar`, `12:30`) keep today's behaviour: they are searched as text.
- A token that is only `-` (or `-""`) is ignored.

Steps:
- [ ] Commit (a): move, `cargo test -p lumina-core search` green with no test changes.
- [ ] Failing tests:
  - `minus_term_excludes_fts_matches`: catalog with "beach.jpg" and "city.jpg"; `-beach` gives only the city row, and `sunset -beach` works. This test fails today.
  - `minus_tag_excludes_tagged_rows`.
  - `negated_booleans_and_aliases_parse`.
  - `unknown_filter_values_are_reported` (`is:favourite` parses as favorite; `is:fovorite`, `color:pink`, `rating:abc` and `year:20x6` land in `unrecognized` and add no clause).
  - `color_none_and_unlabeled_are_one_clause`.
  - `fts_syntax_in_query_text_is_literal` (core_tests: each of `OR`, `NOT x`, `a NEAR b`, `"unclosed`, `a*b`, `^x`, `col:x`, `-`, `-"two words"`, `x OR -y` runs `search_structured` and `search_query_count` without error, and `OR` alone matches a file named "or.jpg", not everything). Pins today's escaping before negation touches it.
  - `exclusion_plan_uses_fts_index` (EXPLAIN contains `VIRTUAL TABLE INDEX` for the `NOT IN` subquery, and the default-sort plan has no `USE TEMP B-TREE FOR ORDER BY`).
- [ ] Implement, then run `cargo test -p lumina-core search`.
- [ ] Bench: seed some bench file names with "beach" (for example every 50th row) so the exclusion has work to do, add `"-beach is:favorite"` under the 5 ms query gate, run `make bench` serialized and alone before and after, and record both numbers in the commit body.

### Task 7: Bench and plan checks for rating and color filters (core, S)

**Files:** `crates/lumina-core/tests/benchmark_test.rs`, `crates/lumina-core/tests/core_tests.rs`; `catalog/schema.rs` + `catalog/mod.rs` only if D9 triggers

- [ ] Seed the bench catalog with realistic spreads after insert, through `Catalog::set_ratings` / `set_color_labels` (`catalog/media.rs:383,457`) in one batch each: ratings 1–5 on ~30% of rows, `color_label` 1 (red) on ~2%, other labels on ~8%. Without this the new queries bench against zero matches.
- [ ] Add bench queries `rating:>=4`, `color:red`, `is:unrated` and `is:unlabeled` to the structured-query gate. Keep the same 5 ms release / 25 ms debug budgets.
- [ ] Add `rating_and_color_filter_plans_use_an_index` (EXPLAIN for each, default date sort, small seeded catalog): the plan uses an `idx_media_` index and does not contain `SCAN m` without one. Record the actual plan in the assertion message. If the planner picks `idx_media_rating` plus `USE TEMP B-TREE FOR ORDER BY`, the bench decides whether that is acceptable; do not force a plan with `INDEXED BY`.
- [ ] Run `make bench` before and after and record both numbers in the commit body.
- [ ] If `color:red` fails the release gate: add the D9 index; bump `SCHEMA_VERSION` to the next free number; idempotent `CREATE INDEX IF NOT EXISTS`; add test `migration_adds_color_index_idempotently` (open twice, `user_version` is the new number, index present, `color:red` plan uses it). Otherwise make no schema change.

### Task 8: Query explanation and suggestion polish (UI, M)

Split into two commits: (a) the move of suggestion helpers with no behaviour change, (b) the new behaviour.

**Files:**
- Create: `crates/lumina-ui-gpui/src/app/search.rs`. Move the suggestion helpers out of `app/library.rs:28-180` and `:894-999` into it, with no behaviour change first. Add the module to `app/mod.rs`.
- Create: `crates/lumina-core/src/search/explain.rs` with `impl StructuredQuery { pub fn explain(&self) -> Vec<String> }`. It returns plain phrases such as "Favorites", "Camera contains “Sony”", "ISO above 800", "Not “beach”", "Not understood: is:fovorite". Phrases are English today; the UI wraps the fixed words through `i18n.rs` keys where practical (AGENTS §11) and the user's tokens are inserted verbatim.
- Modify: `views/search_bar.rs` (explanation footer in the suggestions panel; clock icon on recent rows)
- Modify: `app/command_palette.rs` + `i18n.rs` (`command.clear_recent_searches` "Clear Recent Searches"; applies only when the history is non-empty)
- Modify: `SEARCH_FIELD_SUGGESTIONS` (`library.rs:48`, moved to `app/search.rs`: add `is:photo`, `is:unrated`, `is:untagged`, `is:unlabeled`, `color:red`…`color:purple`, `month:`, `f:`)
- Modify: `app/state.rs` `save_config` returns `bool` (true on success; true under `cfg(test)`) so callers can revert; existing callers ignore it unchanged.
- Test: `crates/lumina-core/src/search/explain.rs` unit, `crates/lumina-ui-gpui/src/tests/browser.rs`

**Interfaces:**
- Selectors: `search-explanation` (footer: `Role::Status` with the joined phrases as `aria_label`, shown while the field is focused and the query is non-empty) and `search-suggestion-recent-{index}`. The second is added on recent rows *in addition to* the existing `search-suggestion-{index}`, which keeps the test contract.
- Clearing history: take the old list, clear, `save_config()`; if it returns false, restore the old list (the banner is already set by `save_config`).

Steps:
- [ ] Failing tests:
  - `explain_phrases_cover_every_field` (one assertion per `StructuredQuery` field, so a new field without a phrase fails).
  - `explanation_lists_not_understood_tokens` (type `is:fovorite sunset`; `search-explanation` contains "Not understood: is:fovorite").
  - `recent_rows_are_marked_and_clearable` (seed two recents; `search-suggestion-recent-0` exists; palette "Clear Recent Searches": dropdown shows no recent rows; `config.recent_searches` is empty in memory; test config is not persisted, per §2).
  - `color_suggestions_offered_for_color_prefix` (typing `color:` offers the six colours).
- [ ] Implement, test, `make ui-smoke`.
- Keyboard path: ↑/↓/Enter in the dropdown (exists), ⌘K "Clear Recent Searches". Docs: create `docs/search.md` (the grammar reference, including negation and its prefix behaviour, `is:photo` = non-RAW, "Not understood", and saved searches) and link it from the README.

### Task 9: Quick filters as query tokens; search scope (UI, M)

**Files:**
- Modify: `views/search_bar.rs` (delete `GridFilter`), `app/render/grid.rs:206-211,364-375` (remove `dimmed` and the `media-card-dimmed-{id}` marker; the trash opacity at `:358` keeps its condition minus `!dimmed`), `app/state.rs:349,551` (remove the `grid_filter` field), `app/command_palette.rs:322-351,670` (replace `Filter(GridFilter)`)
- Modify: `app/search.rs` (`toggle_query_token`)
- Modify: `app/library.rs` `set_search_query` and `apply_search_suggestion_at`, and `app/text_input.rs:568-571` (`text_edited(TextTarget::Search)`): all three apply D11 through one helper `scope_search_to_library(&mut self)` in `app/search.rs`
- Modify: `i18n.rs` (keys `command.filter_*` retitled to "Show Favorites", "Show Videos", "Show Rated ★ N+" for N in 1..=5, "Show Unrated", "Show Color: Red"…"Purple", "Show Unlabeled", "Clear Filters")
- Test: rewrite `tests/browser.rs:111-160` (`filter_chips_select_a_filter_that_dims_rather_than_hides`)

**Interfaces:**
- `pub(crate) fn toggle_query_token(query: &str, token: &str) -> String`. It removes the token if a token with the same parsed effect is present. Tokens in one family replace each other: rating (`rating:*`, `stars:*`, `is:unrated`, `has:rating`), colour (`color:*`, `is:unlabeled`, `has:color`), media type (`is:video`, `is:photo`, `is:raw`). Otherwise it appends the token. Quoted phrases and free text are kept.
- `CommandId::QuickFilter(&'static str)` holds the token. "Clear Filters" removes every token the parser treats as a filter (including negated ones) and keeps free text and `-word` exclusions.
- D11: when the query goes from empty to non-empty, `selection = AlbumSelection::AllLibrary`; the existing navigation recording makes it one Back step (Back restores the album). Typing records navigation through the same helper (today typing records none). A quick filter used inside an album therefore leaves the album; this is the visible cost of D11 (Open questions 3 and 5).

Steps:
- [ ] Failing tests:
  - `toggle_query_token_adds_removes_and_replaces` (unit: `rating:>=3` replaces `is:unrated`; `color:red` replaces `is:unlabeled`; quoted phrases survive).
  - `palette_filter_toggles_query_token` (⌘K, "show rated 4", Enter: query contains `rating:>=4` and the grid count equals `search_query_count("rating:>=4")`; run it again and the token is removed).
  - `no_dimmed_cards_after_filter` (no `media-card-dimmed-*` selector exists anywhere after any quick filter).
  - `typing_a_query_moves_sidebar_to_library` (select Favorites, type "kyoto" through real keystrokes: `selection == AllLibrary`; Back gives Favorites with an empty query).
  - `suggestion_and_command_queries_move_sidebar_to_library` (`set_search_query` from the inspector path and `apply_search_suggestion_at` both reset the selection).
- [ ] Implement, test, `make ui-smoke`.
- Docs: `docs/search.md` "Quick filters"; shortcut sheet unchanged.

### Task 10: Save Search as Smart Album (core + UI, M)

**Files:**
- Modify: `crates/lumina-core/src/smart_albums/rules.rs` (`RuleField::Query`; `compile_rule` branch; the `column_for_field` match gets an `unreachable!` arm like `Tag`)
- Modify: `crates/lumina-core/src/search/mod.rs` (`pub fn build_where_subquery(&self) -> (String, Vec<Value>)`: same as `build_sql`, but the FTS match is an `IN (SELECT rowid …)` subquery; both share one private clause builder so they cannot drift)
- Modify: `crates/lumina-ui-gpui/src/app/smart_draft.rs` (label "Search matches", `ValueKind::Text`, operator list `[Equals]` only; not added to `FIELDS`, shown when present like the legacy fields)
- Modify: `app/album_builder_actions.rs` (`save_current_search_as_album` next to the private `open_draft`, `:55`), `app/command_palette.rs`, `i18n.rs` (`command.save_search` "Save Search as Smart Album…"; applies only when the query is non-empty)
- Test: `smart_albums/mod.rs` unit, `crates/lumina-core/tests/core_tests.rs`, `crates/lumina-ui-gpui/src/tests/smart_album_builder.rs`, bench

**Interfaces:**
- `compile_rule(Query, Equals, Text(q))` parses `q`, rejects `is:trashed` with the error "A smart album cannot show deleted items", rejects any non-empty `unrecognized` with "Not understood: …", rejects an empty query, and returns `build_where_subquery`. Other operators return `Err`. The builder preview and save show these errors through the existing draft error path.
- `save_current_search_as_album(cx)` calls `open_draft(None, <query as title, at most 60 chars, cut on a char boundary>, MatchMode::All, &[Query rule])`. The existing builder sheet confirms and saves, and the existing error banner handles save failures.

Steps:
- [ ] Failing tests:
  - `query_rule_album_matches_search_rows` (seeded catalog, 6 queries including FTS, negation and rating/color: album rows == `search_structured` rows, same order).
  - `query_rule_rejects_trashed_unknown_and_empty`.
  - `query_rule_plan_uses_fts_index` (EXPLAIN: `VIRTUAL TABLE INDEX` for the `IN` subquery, and no `USE TEMP B-TREE FOR ORDER BY` for the default sort).
  - `query_rule_round_trips_rules_json`.
  - UI test `save_search_opens_prefilled_builder_and_creates_album` (type the query, ⌘K "save search", Enter, OK: a new sidebar row shows the same count).
- [ ] Bench: add a `Query`-rule album (`sunset camera:Sony rating:>=3`, using Task 6/7 seeding) to the smart album gate (< 8 ms release). Run `make bench` alone and record the numbers.
- [ ] Implement, test, `make ui-smoke`.
- Docs: `docs/search.md` "Saved searches"; release notes when plan 1 defines where they live (there is no CHANGELOG file today).

### Task 11: Docs refresh and final gate (S)

- [ ] ROADMAP.md §7–§8 "Current", plus the backlog P0 rows for 0.4 (mark what this plan shipped). ux_audit.md §4, §5, §7 "Current" rewritten from the verified state above.
- [ ] `docs/accessibility.md`: list the new roles and labels and say they are not VoiceOver-verified unless they were checked by hand.
- [ ] Full gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `make bench` (alone), `make ui-smoke`, `LUMINA_REQUIRE_UI_PERMISSIONS=1 make ui-e2e` (needs Accessibility permission; report SKIP honestly if it is missing).
- [ ] Independent review per AGENTS.md §9 (the reviewer does not edit). Fix the findings, re-check, and confirm a clean tree.

---

## Open questions for the owner

1. **Quick filter UI.** D8 makes filters query tokens reachable from ⌘K and the suggestions. Should there also be one toolbar "Filter" menu button (rating ★1–5, colours, favorites, videos)? It would be a single control, not the removed chip bar.
2. **Trackpad scroll in a zoomed viewer.** Keep scroll = zoom (current), or move to scroll = pan when zoomed with pinch = zoom (the Photos convention)?
3. **Search scope.** Plan D11 searches the whole library and moves the sidebar to Library. Do you want "search within the selected album/tag" instead? That costs a combined SQL path and another perf gate.
4. **Digit keys (shared with plan 5 Open question 5).** OK to move the viewer presets to `⌘0/⌘1/⌘2/⌘4` with Fill on `⌘3` in this plan (Task 2), so plan 5 can give plain digits to ratings? If not, presets stay on `0/1/2/4` and Fill has no key.
5. **Dimming removal.** Removing `GridFilter` dimming changes a behaviour you designed ("positions and selection stay put"), and with D11 a quick filter picked inside an album now shows library-wide results. Is that acceptable?
6. **Jump-to-date.** It is on the ux_audit UX-2 list, but no plan covers it (plan 2 defers it to UX-2). Today's paths are the year rail and `year:`/`date:`. Should it get its own task here or a separate plan?

## Owner answers (2026-11-06)

1. **Quick filter UI:** no toolbar Filter button (default; D8 as written).
2. **Zoomed scroll:** scroll pans. This replaces D2: in the viewer, two-finger scroll (and a mouse wheel) pans while zoomed in and does nothing at Fit; pinch zooms (Task 1), ⌘-scroll zooms anchored at the pointer (so a mouse wheel can still zoom), and `+ -`/presets stay. Task 1 does this together with pinch.
3. **Search scope:** whole library (D11 as written).
4. **Digit keys:** presets move to ⌘0/⌘1/⌘2/⌘4 with Fill on ⌘3 (D3 as written).
5. **Dimming:** replace it with query tokens (D8 as written).
6. **Jump-to-date:** not in this plan (default); a later plan.

## Out of scope

- Animated grid reflow (D5); pan inertia; smart zoom (two-finger double-tap, not delivered by GPUI); tiled or full-resolution 1:1 decodes (the 2048 px viewer decode stays).
- A View > Zoom menu: presets are viewer-only commands, reachable by key and palette (AGENTS §11 "when appropriate").
- Exact-frame hover previews, a disk cache for strips, buffered ranges (local files are fully available), chapters, and audio/subtitle tracks.
- A volume slider and persisted volume (no volume control exists; Task 5b persists speed and mute).
- Value autocomplete for cameras, lenses and cities (D12); natural-language or semantic search (not implemented, see AGENTS.md §15).
- Jump-to-date (Open question 6).
- Mixed-value batch editing, tag context menus and smart-album previews (other UX-2 rows, planned separately).
- Touch-screen pinch (`gpui-pre` does not recognise it, `gestures.rs:527`; macOS trackpads are unaffected).

## Review notes (2026-11-05)

Independent review against the code (no builds or tests run). Changes:

1. **Digit-key contract:** aligned with plan 5 Decision 13. Task 2 now moves presets to `⌘0/⌘1/⌘2/⌘4`, gives Fill `⌘3`, updates the shortcut sheet (`render/mod.rs:153`), the HUD tooltip (`stage_controls.rs:30`), the existing keyboard test (`viewer_and_video.rs:564`), and adds `cmd_digit_zoom_presets_in_viewer` / `plain_digits_do_not_zoom_viewer`. Old Open question 4 rewritten as the shared decision; fallback documented. Verified no `⌘digit` binding exists today.
2. **Cross-plan conflicts added:** plan 5 extends `GridFilter` and reuses its dim opacity, which Task 9 deletes; plan 4's hierarchical tags affect `-tag:`; schema version numbering.
3. **Pinch facts:** macOS never sends `TouchPhase::Cancelled` (removed from the reset rule); `simulate_event` citation corrected to `test_context.rs:1068`; hover requirement and `mouse_position` update noted.
4. **Grid ⌘-scroll** steps per event with no accumulation: a trackpad burst would jump the ladder. Added pixel accumulation and a test; moved the accumulator out of `GridState` (wrong struct) into a small `GridZoomGesture`.
5. **FFI memory safety:** the proposed C API wrote into caller memory from async callbacks; after a timeout that is a use-after-free. Now mirrors `LuminaFrameGrab` (holder object, copy after wait).
6. **Strip cancellation / failure:** added a cancel flag, one in-flight request, no retry after failure, skip when the player could not load, `retire_image` eviction, frame count scaled to duration, memory corrected (~7 MB, ≤12.6 MB), undecodable-file test.
7. **Wrong names:** `VideoState` → `VideoPlaybackState`; `cx.background_spawn` → `cx.background_executor().spawn`; "stub player" does not exist; `open_draft` lives in `album_builder_actions.rs`; `is:fav` already exists; no CHANGELOG file.
8. **D11 hole:** typing bypasses `set_search_query` (`text_input.rs:568`); Task 9 now covers all three paths plus a test.
9. **Search:** added FTS5 syntax/injection regression test, prefix semantics of `-term`, unparseable `year:`/`date:` reporting, token families in `toggle_query_token` (`is:unrated` vs `rating:`).
10. **Bench realism:** the bench catalog has no ratings, labels or matching words; Tasks 6/7 now seed them. Task 7's EXPLAIN assertion no longer contradicts itself (rating index vs no temp b-tree).
11. **Error path:** Clear Recent Searches reverts on a failed write (`save_config` returns `bool`).
12. **Dropped requirements surfaced:** persisted speed/mute (new Task 5b, small; volume has no UI); jump-to-date (Open question 6, Out of scope).
13. **File size:** scrub code goes into new modules (`video_actions.rs` 624, `transport.rs` 606 lines); Tasks 6 and 8 split into move + behaviour commits.
14. Stale citations fixed: `viewer_actions.rs:188`, `command_palette.rs:310-321`, `viewport.rs:18`, `search.rs` line ranges, `panels.rs` ranges.

Not verified: real trackpad pinch feel; whether AVFoundation honours `cancelAllCGImageGeneration` promptly for HEVC; actual `EXPLAIN` output for the `NOT IN` FTS subquery (the expected `VIRTUAL TABLE INDEX` wording should be confirmed on first run); (since verified with ffprobe: fixture `clip.mp4` is 2.0 s, 64×48 H.264, so Task 4's strip test must expect frames in 0..2 s); whether `ui_automation_plain_wheel_scrolls_and_cmd_wheel_zooms_around_pointer` uses pixel deltas under 40 px.
