# Housekeeping review

Date: 2026-11-20 · Tree: HEAD `1c27102` (audits started at `39f0eea`; only CI/docs/scripts landed since, no Rust source)
Scope: all production Rust in `lumina-core` and `lumina-ui-gpui`, plus tests, scripts, packaging and repo layout.
Method: read-only static audit by three parallel reviewers with disjoint areas, plus a Lead pass over
scripts, tests, manifests and repo artefacts. No source was changed. Every finding cites `file:line`;
spot checks by the Lead confirmed the items marked **verified**.

Working papers (full evidence, "checked and clean" and "do not touch" lists):
`.agents/housekeeping/core.md`, `ui-app.md`, `ui-views.md`.

**Bottom line:** the tree is clean where it counts. There are **0 production `unwrap()`/panic paths**,
no TODO/FIXME markers, no unused dependencies, no unreachable modules and only 4 narrow clippy allows.
The real debt is **duplication that can drift** (SQL predicates, overlay scaffolds, reset sequences),
**oversized mixed-concern modules** (12 in the UI, 10 in core), and an **unindexed known-deviation
register** (115 `ponytail:` comments). Five items are correctness/data-safety adjacent; everything else
is drift prevention or review cost.

Findings: **74** — 5 correctness/data-safety, 23 duplication, 31 oversized/mixed modules, 15 hygiene.
Most duplication and hygiene items are XS/S and batch safely; the 31 module splits are M/L and should
be done when the file is next touched (or as isolated, no-behaviour-change PRs).

---

## 1. Do first: correctness and data-safety (5)

| ID | Finding | Evidence | Why it matters | Effort |
|----|---------|----------|----------------|--------|
| CORE-21 **verified** | `promote_legacy_catalog` checks the catalog rename but swallows the `-wal`/`-shm`/`.bak` renames; a failed WAL move can leave committed frames behind on the promoted DB. | `crates/lumina-core/src/library.rs:66,74-79` | AGENTS §2: user data never silently lost. Edge path, but invisible when it fails. | XS + test |
| H14 **verified** | The "forget the old library" reset is copy-pasted with different order/lists: `app/library.rs:184-216` vs `app/library_delete.rs:176-212`. | same | A field missed on one path leaves stale ids/undo/textures that can act on the new library's rows. Extract `reset_library_state()` + a field-list regression test. | S/M |
| UI-02 **verified** | `album_page::render_menu_backdrop` is the only menu backdrop without `.occlude()`; the downloads twin has it. | `views/album_page/mod.rs:261-276` vs `views/downloads_screen.rs:338-355` | Same click-leak class fixed in the earlier overlay sweep: a dismiss click can also hit the chip behind. Verify with a rendered click test. | XS |
| UI-09 | Add-filter menu does `expect` on a hand-maintained static table; a new token panics the render path. | `views/filter_bar.rs:83` (table `:40-55`) | AGENTS §5.2: `expect` is for startup invariants. Replace with a fallback + unit test over `FIXED_ROWS`. | XS |
| H23 **verified** | `is_some_and(...)` then `.expect("open library")` in the library-delete path. | `app/library_delete.rs:51-56` | Cannot panic today, but the check/use pair can drift. Convert to `let ... else`. | XS |

## 2. High-drift duplication (do as one mechanical wave)

These are the changes that quietly make two code paths disagree later. None changes behaviour.

| ID | Finding | Evidence | Copies | Effort |
|----|---------|----------|--------|--------|
| CORE-05 | Page + count SQL template copy-pasted instead of `sql.rs`. | `catalog/media.rs:371`, `catalog/tags.rs:283`, `catalog/duplicates.rs:137`, `search/mod.rs:410,443`, `smart_albums/rules.rs:179`, `smart_albums/system.rs:129` | 6 | S/M |
| CORE-06 | "File present" predicate hand-written although `files::PRESENT` exists; "active item" predicate has no constant. | `catalog/files.rs:18`; raw copies `catalog/duplicates.rs:25,29,81,96,115,119,142,146,170,197,201` | 11 + 16 | S |
| UI-01 | Six-plus modals hand-roll `scrim + occlude + modal_dim + stop_propagation + bg_1/line_2/RADIUS_LG/shadow_2xl`. Crate-wide: 52 `.occlude()`, 10 `modal_dim()`, 17 `shadow_2xl()`, 7 identical stop-propagation closures. | `views/library_delete_modal.rs:19-53`, `album_picker.rs:54-86`, `import_dialog.rs:102-134`, `smart_album_delete.rs:65-95`, `downloads_confirm.rs:28-56`, `delete_modal.rs:227-256` | 6-8 | M |
| UI-04 **verified** | The six colour labels are encoded three times: theme colours, `label_options()`, and the match in `get_item_color_label`. | `theme/mod.rs:130-135`, `theme/controls.rs:221-231`, `app/selection.rs:530-542` | 3 | S |
| H13 **verified** | Two hand-written lenient deserializers duplicate the generic `lenient`; both enums derive `Default`, so the generic is identical. | `config.rs:565-574,587-595` vs generic `config.rs:518-525` | 2 | XS |
| H15 | Welcome and library roots duplicate the shell, the settings/about handlers and the overlay stack. | `app/render/welcome.rs:33-41,19-29,94-100` vs `app/render/mod.rs:87-96`, `render/library_actions.rs:60-70`, `render/mod.rs:343-354` | 2 | S |
| H16 | Three batch-field summarizers repeat the first/mixed scaffold. | `app/batch_edit/mod.rs:259-272,288-305,309-326` | 3 | XS |
| H17 | `%Y-%m-%d %H:%M:%S` magic format repeated for batch and inline editing. | `app/inline_edit/date_location.rs:26-28`, `app/batch_edit/mod.rs:328-332`, parse `app/inline_edit/parse.rs:93` | 3 | XS |
| H18 | Month names hardcoded in `month_name` although i18n has month keys; search month tokens re-listed in the UI. | `app/types.rs:285-301`; `i18n/en_us.rs:740-751`; `app/search/top_match_open.rs:11-13` vs `core/search/parse.rs:245` | 2 | XS |
| UI-05 **verified** | `floating_pip::clock` reimplements `format_clock` without the hours branch (a 90-minute clip shows `90:00`, not `1:30:00`); three views format dates with raw chrono. | `views/floating_pip.rs:23-27` vs `app/types.rs:234-251`; `inspector/tabs/info.rs:315`, `network_import_dialog.rs:15`, `downloads_popover_rows.rs:22` | 4 | XS |
| CORE-08 | Two RAII scratch-dir guards. | `decoder.rs:24-40`, `net_import/mod.rs:246-273` | 2 | S |
| CORE-09 | Timeline rebuild runs the same INSERT three times. | `catalog/schema.rs:746-797` | 3 | S |
| CORE-10 | `write_jpeg` duplicated in two integration tests. | `tests/import_test.rs:98`, `tests/watcher_test.rs:55` | 2 | XS |
| CORE-07 | Two child-process wait loops with different kill strategies. | `decoder.rs:149-183`, `net_import/mod.rs:360-384` | 2 | M |
| H19 | Album/smart-album overview card menus and arrow-step math duplicated. | `app/albums_overview.rs:293-306,438-452` vs `app/smart_cards.rs:88-101,105-118` | 2 | XS/S |
| H20 | Clear-completed / clear-queue duplicate take-partition-close. | `app/downloads/panel.rs:396-405`, `app/downloads/screen.rs:102-111` | 2 | XS |
| H21 | Month-band and bucket-card title rows duplicate typography. | `app/render/bucket_cards.rs:309-330`, `app/render/grid.rs:287-309` | 2 | XS |
| UI-03 | Overview card menu + caption builders duplicated. | `views/albums_overview/albums.rs:389-431,344-372` vs `smart.rs:351-395,220-264` | 2 | S |
| UI-06 | Video HUD speed/audio pills hand-roll the control shape. | `views/viewer/transport_controls.rs:138-165,375-397` vs `theme/controls.rs:9-31` | 2 | XS |
| HK-01 **verified** | Version extraction has 3 slightly different implementations and `cleanup()` has 4 variants across 13 shell scripts (1,553 lines, no shared lib). | `scripts/build_release.sh:20-21`, `scripts/package_release.sh:16`, `scripts/package_linux.sh:17-21`; `cleanup()` in `live_ui_smoke.sh:207`, `live_ui_automation.sh:129`, `linux_package_smoke.sh:30`, `wayland_smoke.sh:50` | 3 + 4 | S |
| HK-02 **verified** | `tests/helpers.rs` already provides `bounds_center`/`click`/`render_ui`/…, yet 130 test files redefine local `bounds` (9), `toast` (5), `width` (4), `shown` (4), `settle` (3), `select_first` (3). | e.g. `tests/filter_bar.rs:14`, `tests/video_player.rs:37`, `tests/album_grid.rs:31`, `tests/compare.rs:31` | ~28 | S |
| CORE-25 / H24 | Two narrow clippy allows lack the justification comment §5.1 requires. | `core/types.rs:148`, `app/types.rs:104` | 2 | XS |

## 3. Oversized / mixed-concern modules (split when touched, not in one sweep)

Production lines exclude `#[cfg(test)]`. AGENTS §5.1 asks to split past ~500 lines or mixed concerns.
Full seam suggestions are in the working papers; the intended order is pure-helper extraction first
(geometry, metrics, parsing, SQL) because those are testable without the UI.

| ID | Module | Production lines | Recommended seam | Effort |
|----|--------|------------------|------------------|--------|
| H01 | `app/state.rs` | 1,202 (127 flat fields, ~220-line ctor) | `state/{tags,video,albums,search,grid,session}.rs`; start by moving the ctor | L |
| CORE-11 | `core/lib.rs` | 988 (~657 one-line delegates) | move wrappers into existing `engine/*`; add crate docs in the same pass | M |
| H02 | `config.rs` | 948 | `config/{types,defaults,store,restore,resume}.rs` | L |
| CORE-14 | `decoder.rs` | 894 | `decoder/{mod,platform_tools,macos,linux}.rs`; remove the duplicated `native_pixel_size` | L |
| CORE-12 | `catalog/schema.rs` | 834 | `schema/{mod,migrations}.rs` + `catalog/timeline.rs`; split before the next migration | L |
| CORE-13 | `catalog/media.rs` | 731 | extract annotations to `catalog/annotations.rs` | M |
| H03 | `app/text_input.rs` | 727 | `text_input/{target,field,actions}.rs` | M |
| H04 | `app/video_actions.rs` | 706 | `video_actions/{settings,transport,scrub}.rs` | M |
| H05 | `app/preferences.rs` | 715 | `preferences/{tabs,cache,docs,actions}.rs` | M |
| UI-07 | `views/grid.rs` | 673 | `views/grid/{metrics,zoom,state}.rs` (imports change across `app/render/*`) | M |
| H06 | `app/downloads/mod.rs` | 666 | `downloads/{queue,actions}.rs` (siblings already split) | M |
| H07 | `app/map.rs` | 668 | `map/{basemap,state,actions}.rs` | M |
| CORE-15 | `scanner.rs` | 556 | `scanner/exif.rs` (the `mp4` submodule sets the pattern) | M |
| CORE-16 | `video_frame.rs` | 554 | `video_frame/{mod,ffi,macos,linux}.rs` | M |
| UI-08 | 8 modules (group) | 514-572 | `player/linux`, `context_menu`, `floating_pip`, `inspector/body`, `inspector/batch`, `downloads_rows`, `onboarding`, `i18n/en_us` | M each |
| H08-H12 | 5 modules | 434-585 | card badges, empty-state copy, toolbar window controls, rail geometry, recents | S/M each |
| CORE-17..20 | 4 modules | 513-521 | `net_import/{url,process}`, `duplicates/{queries,merge}`, `search/sql`, `catalog/keyword_aliases` | M each |

## 4. Hygiene nits and repo artefacts

| ID | Finding | Evidence | Action |
|----|---------|----------|--------|
| CORE-01 **verified** | `library::remove_library` has no callers (the UI uses `remove_library_at`). | `core/library.rs:102-104` | Delete the wrapper. |
| CORE-02 | `all_file_signatures` is test-only but documented as a production need. | `catalog/media.rs:274-280`; callers `library.rs:652,670` | Gate with `#[cfg(test)]` or delete with its tests. |
| CORE-03 **verified** | `DuplicateCandidate.matched_item_path` is written 6× and never read. | `duplicates.rs:41`; writes `engine/duplicates.rs:203-275` | Remove the field (keep `matched_item_id`). |
| CORE-22 | `create_dir_all` ignored before QuickLook decode, hiding the real failure. | `decoder.rs:722-724` | Add `with_context(...)?`. |
| CORE-23 | Thumbnail-cache cleanup thread spawn result ignored. | `core/lib.rs:160-181` | Log on failure. |
| UI-10 | Five comments point at module paths that are now directories. | `views/preferences/general.rs:53`, `downloads.rs:54`, `viewer/edit_bar.rs:3`, `template_editor.rs:3`, `stacks.rs:2` | Fix paths (`app/keymap/modals/`, `views/duplicates/`). |
| H25 | Comment cites a planning artefact ("Review Focus 7"). | `app/render/mod.rs:81` | Reword to the invariant. |
| UI-11 | 36 visible strings bypass the i18n catalog (410 `t()` calls elsewhere). | e.g. `views/sort_menu.rs:105,131`, `delete_modal.rs:182,405`, `marks.rs:89` | Add keys per file when touched. |
| UI-12 | `player/dummy.rs` is compiled only for a configuration no CI builds. | `player/mod.rs:9-10`; `player/dummy.rs` | Decide with the Windows plan (keep with a comment, or delete). |
| UI-13 | 32 `pub`/`pub(crate)` items are used only in their own file. | `views/grid.rs:67,70,95,98,143,172,175,300,510` etc. | Narrow visibility opportunistically. |
| H22 | ~101 lines of test fixtures live in the production `app/types.rs`. | `app/types.rs:186-221,303-366` | Move to `src/tests/fixtures.rs`. |
| CORE-04 | Six `Catalog`/`DiskCache` methods exist only for tests but are public API. | `catalog/albums.rs:142`, `smart.rs:12,108`, `mod.rs:230,285`, `cache.rs:86` | Decide: `#[doc(hidden)]` or move under `test-support` (CI would need the feature). |
| UI-16 | Four player getters are test-only but compiled into release behind a justified allow. | `player/backend.rs:36-45` | Optional: gate with `#[cfg(test)]`. |
| HK-03 **verified** | `lumina.pen` is a 2.1 MB tracked design file at the repo root, referenced by nothing; it is the largest tracked file (≈35% of repo bytes). | `git ls-files lumina.pen`; no references in docs/scripts/Makefile | Confirm whether it is meant to ship with the source; otherwise drop it from tracking. |
| HK-04 **verified** | 115 production `ponytail:` comments record known deviations, with no index and no review trigger. | `grep -rn ponytail crates` (86 UI, 32 core, 5 test) | Generate `docs/known-deviations.md` from them (file:line + text) and prune on touch; consider an owner/exit criterion for the important ones. |

## 5. Checked and clean (do not re-investigate)

- **No production `unwrap()`** in either crate; 0 `panic!`; 3 `expect` sites total, two legitimate invariants (UI-09 and H23 are the two to fix; `config.rs:751` is startup).
- **No TODO/FIXME/HACK/XXX**; no `dbg!`; no `todo!`/`unimplemented!`.
- **No unused dependencies** (all 15 core and 13 UI deps have call sites, including `exif` and `async-channel`), no dead feature flags.
- **Only 4 clippy allows**, all narrow (`large_enum_variant` ×2, `too_many_arguments` ×2); two lack the §5.1 justification comment.
- **No unreachable modules**; every `pub mod` is reachable; the only fully unreferenced function is CORE-01.
- **No unused i18n keys** (871 keys; 25 apparent gaps are dynamically produced families), no unused icons (96) or theme tokens (61).
- **Config fields are all read**; no swallowed catalog/config/sidecar writes (the `let _ =` sites are channel sends, thread joins, temp cleanup and UI updates).
- **Duplicate `debug_selector` literals exist but are safe**: `filter-bar`/`filter-clear-all`/`filter-match-count` (grid vs video filter bars), `sidebar` (welcome vs library), `inspector-filename` (floating window vs docked panel) — verified mutually exclusive or in different windows. Do not "fix" them without checking exclusivity.
- **Deliberate separations**: `selection` vs `selection_by_id`, `crop` vs `crop_geometry`, the mosaic trio, `dims.rs` vs `catalog/dims.rs`, the three duplicate layers, `ResumePositions`' custom serde, the `text_marker`/`CARD_OPACITY`/`TEST_WINDOW_MOVES` test hooks, per-platform copy pairs, and the `cfg(windows)`/non-macOS stubs.

## 6. Suggested execution

**Wave A — safe mechanical batch (one PR, ~20 XS items).** No behaviour change, existing tests are the guard:
CORE-01, CORE-02, CORE-03, CORE-10, CORE-22, CORE-23, CORE-25/H24 (comments), H13, H16, H17,
H20, H21, UI-05 (clock only), UI-06, UI-10, H25, HK-01, HK-02.
Gate: fmt, clippy, full tests.

**Wave B — isolated behaviour fixes (one PR each, each with a regression test).**
UI-02 (occlude + rendered click test), UI-09 (fallback + table test), H23, CORE-21 (WAL rename + test),
H14 (shared reset + field-list test). H14 and CORE-21 are data-safety: do them before any library
feature work.

**Wave C — the two shared helpers worth their abstraction.**
UI-01 (`views/overlay.rs`: `scrim` + `dialog_card`, migrate 6-8 sites, verify dismissal), then
CORE-05/CORE-06 (`sql::page_query`/`count_query`, `files::present`/`ACTIVE`). Both are multi-file
but mechanical; the overlay one needs rendered dismiss checks per site.

**Wave D — oversized modules, only when touched.** Start with pure extractions that carry tests:
`views/grid/{metrics,zoom}` (UI-07), `catalog/media` annotations (CORE-13), `scanner/exif` (CORE-15),
`search/sql` (CORE-19). Leave `state.rs`, `config.rs` and `schema.rs` for dedicated, isolated changes.

**Wave E — decisions before work.** CORE-04 (test-support API policy), UI-12 (dummy player/Windows),
UI-15 (i18n split), HK-03 (`lumina.pen`), HK-04 (deviations index).

## 7. Open decisions

1. **Test-only public API (CORE-04):** keep public with `#[doc(hidden)]`, or move under the existing
   `test-support` feature? CI would need `--features test-support`; decide once, not per item.
2. **`player/dummy.rs` (UI-12):** keep for the planned Windows target, or delete until that port starts?
3. **`lumina.pen` (HK-03):** keep the design file in the source repo (2.1 MB), or track it elsewhere?
4. **Module splits:** do we want dedicated split PRs (reviewable, no behaviour change), or only
   split-on-touch? The list above is ordered for the latter.
5. **Known deviations (HK-04):** is a generated `docs/known-deviations.md` wanted, and should entries
   carry an owner/exit criterion, or stay as free-form `ponytail:` comments?
---

## Status (2026-11-20)

Four commits landed on top of `1c27102`:

- `bfabb27` Wave A — mechanical: CORE-01/02/03/10/22/23/25, H13/16/17/20/21/23/24/25,
  UI-05/06/10/13, HK-01 (scripts/lib/common.sh).
- `a310789` Wave B — behaviour: UI-02 (album menu backdrop occludes), UI-09 (filter
  menu fallback + table test), CORE-21 (WAL companion rename logged + test), H14 (shared
  library reset).
- `8d86b26` Wave C-lite — CORE-04/24/28/30, UI-12, HK-04 (docs/known-deviations.md).
- `7e85bdd` UI-04 — one source for metadata label colours.

Gates at the end: `cargo fmt --all -- --check` clean; `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean; `cargo test --workspace` 1396 UI tests
plus every core suite pass; `make ui-smoke` PASS. One test is **not** a pass:
`engine_lifecycle::dropping_the_engine_stops_its_threads` is skipped because this sandbox
denies `ps -M` (Operation not permitted); it is untouched by these commits.

### Not done (still open in this file)

31 oversized-module splits (H01-H12, CORE-11..20, UI-07/08) are multi-file moves with
wide import churn; each needs its own reviewed change. Also open: CORE-05/06/07/08/09
(SQL and helper duplication), CORE-26/27 (naming/public renames), CORE-29, H15/18/19/22,
UI-01 (overlay helper), UI-03, UI-11 (36 hard-coded strings), UI-14/15/16, HK-02 (test
helper consolidation), HK-03 (`lumina.pen` is a product decision).

### Review limitation

AGENTS.md §9 requires an independent reviewer. In this session every teammate failed to
start, including a one-word probe, so each slice was reviewed by the author only and the
commit bodies say so. Before merging upstream, re-review `bfabb27..7e85bdd` with a
second agent and re-run the gates.