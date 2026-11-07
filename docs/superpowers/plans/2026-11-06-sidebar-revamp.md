# Sidebar Revamp Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three changes to the sidebar:
- one clear toggle for hiding and showing the sidebar;
- an icon on every default collection;
- a collapsed sidebar that becomes a narrow rail of those icons, so you can still switch collections without opening it.

**Architecture:** The sidebar already collapses to a 48 px column (`SIDEBAR_COLLAPSED_WIDTH`, `views/grid.rs:36`), but the column holds only an expand button (`views/sidebar/mod.rs:50-86`).
- The default collections become one ordered list of `NavEntry { key, label, icon, selection, count, visible }`, built once per render in a new `views/sidebar/nav.rs`.
- The expanded rows and the collapsed rail both render from that list, so the two views cannot drift apart.
- Icons come from the existing `Icon` set, plus a few new Lucide SVGs added to `resources/icons/`.
- The toggle stays in the toolbar. It gets a state-aware icon and label, and a keyboard shortcut.

**Tech Stack:** Rust 2021, GPUI Kit 0.9 (`gpui-pre` 0.3.11), the existing Lucide icon pipeline (`icons.rs`, `assets.rs`). No new crates.

## Current state (verified 2026-11-06)

- **Collapse entry points.** Every one calls `LuminaApp::toggle_sidebar` (`app/panels.rs:27`):
  - the toolbar button `sidebar-toggle` (`app/render/toolbar.rs:128`, `Icon::PanelLeft`, tooltip "Toggle sidebar");
  - a second button, `expand-sidebar`, inside the collapsed column;
  - View > Toggle Sidebar (`menus.rs:29`);
  - ⌘K "Toggle Sidebar", which has no shortcut.

  There is no keyboard shortcut. The setting is saved as `config.sidebar_visible`.
- **Default collection rows.** Each is a plain text row with a count (`render_nav_item`, `views/sidebar/sections.rs:595`, 26 px). None has an icon, a tooltip or a role. The rows are:
  - All Photos & Videos, Favorites, Videos, Screenshots, Places, Duplicates and Recently Deleted;
  - Downloaded and Missing, shown only when their count is above 0;
  - Untagged, under Smart Albums.
- **Icon names already exist but are unused.** `SystemAlbum::icon_name()` (`crates/lumina-core/src/smart_albums/system.rs:54`) returns a Lucide name per system album; the UI never reads it.
- **Bundled icons** (`resources/icons/`, 40 in total). These are relevant:
  - image, heart, film, arrow-down-to-line, trash-2, triangle-alert, tag, sparkles, panel-left, folder.

  These are missing:
  - images, map-pin, copy, camera (or monitor), panel-left-close, panel-left-open.
- **Test contract.**
  - `sidebar-nav-{label}` is used by about 12 tests.
  - `expand-sidebar` is used by `ui_automation_toolbar_buttons_by_mouse` (`tests/ui_automation.rs:196`).
  - `test_grid_row_height_tracks_window_and_panel_chrome` asserts the 48 px collapsed column with no resizer (`tests/windows_and_layout.rs:311`).
  - `sidebar_rows_are_26px` (`tests/design_system.rs:68`).

## Decisions

- **S1. One toggle, in the toolbar.**
  - The rail's own `expand-sidebar` button is removed. The toolbar button is always at the same place (after the traffic lights), so the toggle never moves.
  - Its icon reflects the state: `panel-left-close` when open, `panel-left-open` when collapsed. Its tooltip and aria label say "Hide Sidebar (⌃⌘S)" or "Show Sidebar (⌃⌘S)".
  - The View menu item and the palette entry use the same titles.
  - *Rejected:* keeping a second toggle in the rail. Two controls for one action is what made it feel unclean.
- **S2. Shortcut ⌃⌘S.** This is the macOS standard for Show/Hide Sidebar (Finder, Mail, Notes, Xcode). It goes in the View menu, in the palette's shortcut column and on the shortcut sheet. It must be absorbed while a modal is open (AGENTS §6.3).
- **S3. One list drives both views.** A `NavEntry` list in `views/sidebar/nav.rs` holds the stable `key` (e.g. `favorites`), the label, the `Icon`, the `AlbumSelection`, the count and the visibility rule.
  - The expanded rows keep their label-based selector `sidebar-nav-{label}`, so the existing tests keep working.
  - The rail uses key-based selectors: `sidebar-rail-{key}`.
- **S4. Icons on every default collection.** These are monochrome Lucide icons at 16 px. They use `text_row` when idle and `selection_fg` when active, matching the label.

  | Row | Icon | Status |
  |---|---|---|
  | All Photos & Videos | `images` | new |
  | Favorites | `heart` | bundled |
  | Videos | `film` | bundled; the core name "video" maps to it |
  | Downloaded | `arrow-down-to-line` | bundled |
  | Screenshots | `camera` | new |
  | Places | `map-pin` | new |
  | Duplicates | `copy` | new |
  | Missing | `triangle-alert` | bundled |
  | Recently Deleted | `trash-2` | bundled |
  | Untagged | `tag` | bundled |

  Smart-album rows get `sparkles`. Tag rows keep their colour dot. Row height stays 26 px.
  - The mapping lives in the UI (`nav.rs`). `SystemAlbum::icon_name()` stays core-only metadata.
  - *Rejected:* reading the core's string names, which would need a string-to-`Icon` lookup for one caller.
- **S5. The collapsed rail.**
  - Same 48 px width, with `SIDEBAR_COLLAPSED_WIDTH` used everywhere instead of the hard-coded 48.
  - One 32×32 icon button per visible default collection, in the same order, with no counts. The active collection uses the `selection_bg` pill.
  - Each button has a tooltip, "Favorites · 22", and an aria label, "Favorites, 22 items", and calls the same `select_album` as the row.
  - Smart albums and tags do not appear in the rail (see Open question 3).
  - Downloaded and Missing follow the same "count above 0" rule as the rows.
- **S6. No collapse animation in this plan.** GPUI 0.3.11 has no width transition. Faking one would mean animating `grid_metrics` every frame, which re-lays out the grid every frame.
  - The change is instant, as today.
  - *Deferred:* a 160–240 ms ease-out that respects `reduce_motion()`, if the owner wants it (Open question 2).
- **S7. Accessibility.** Rail buttons and nav rows get an `id`, `.role(Role::Button)` and an aria label. Plan 6 Task A1 later turns the sidebar into a `Navigation`/`Tree`, and this work does not block that. Do not claim VoiceOver support without a hand check.

## Global Constraints

- No new crates and no schema change.
- Icon SVGs are added by hand at dev time: copy the Lucide (ISC-licensed) files of one pinned version into `resources/icons/`, add each to `Icon`, `Icon::ALL`, `path()` and the `lucide!` list (`assets.rs:15`), and keep `every_icon_resolves_in_asset_source` green (black paint, no `currentColor`). Nothing is fetched at run time.
- UI strings go through `src/i18n.rs`: the toggle titles, and the rail tooltip and aria-label formats.
- Every new control gets a `debug_selector`, an `id`, a tooltip and a role plus label.
- `views/sidebar/sections.rs` is 660 lines. The nav list and the rail go in new files (`views/sidebar/nav.rs`, `views/sidebar/rail.rs`). `sections.rs` only shrinks.
- In modules that glob-import `gpui::*`, use `#[core::prelude::v1::test]`. UI tests use `crate::tests::TestApp::single()`.
- Do not touch `crates/lumina-core/src/net_import/*` or `docs/network-import.md`. Stage explicit paths, and never stash.

## Tasks

### Task 1: Icons (S)

- [ ] Add the six Lucide SVGs: `images`, `camera`, `map-pin`, `copy`, `panel-left-close`, `panel-left-open`. Record the Lucide version in `resources/icons/LICENSE` or alongside it.
- [ ] Wire each one into `Icon`, `Icon::ALL`, `path()` and `assets.rs`.
- [ ] Test: the existing `every_icon_resolves_in_asset_source` covers the new icons. Run it.

### Task 2: One nav list, icons on rows (M)

- [ ] Create `views/sidebar/nav.rs` with `NavEntry` and `nav_entries(app) -> Vec<NavEntry>`. It must keep today's order and visibility rules exactly, including that "All Photos & Videos" also clears the active year.
- [ ] Change `render_nav_item` to take an `Icon`, and render the Library rows and the Untagged row from `nav_entries`. Smart-album rows get `sparkles`.
- [ ] Failing tests:
  - `every_default_collection_row_has_an_icon`: for each visible entry, `sidebar-nav-{label}-icon` is present.
  - `nav_entries_keep_order_and_conditional_rows`: Downloaded and Missing appear only with a count above 0, and the order matches the table above.
- [ ] Keep `sidebar_rows_are_26px` and all `sidebar-nav-*` tests green.

### Task 3: Collapsed rail (M)

- [ ] Create `views/sidebar/rail.rs`, which renders the 48 px rail from `nav_entries` as described in S5. Remove the `expand-sidebar` button.
- [ ] Failing tests:
  - `collapsed_rail_shows_an_icon_per_default_collection`: a `sidebar-rail-{key}` exists for each visible entry, and none exists for smart albums or tags.
  - `rail_icon_selects_collection_and_marks_it_active`: clicking `sidebar-rail-favorites` gives `System(Favorites)`, and the grid count equals the Favorites count.
  - `rail_tooltip_and_label_carry_the_count`.
- [ ] Update `ui_automation_toolbar_buttons_by_mouse`: the rail no longer has an expand button, so the test expands with the toolbar toggle. Keep the 48 px assertion in `windows_and_layout.rs`.

### Task 4: Cleaner toggle and shortcut (S)

- [ ] Make the toolbar toggle state-aware (icon, tooltip and aria label, per S1). Give the View menu item and the palette entry "Hide Sidebar" / "Show Sidebar" titles with ⌃⌘S. Add a `KeyBinding` for ⌃⌘S and put it on the shortcut sheet.
- [ ] Failing tests:
  - `ctrl_cmd_s_toggles_the_sidebar_and_persists`: in memory, without writing real config.
  - `sidebar_shortcut_is_absorbed_by_modals`: with a modal open, ⌃⌘S does nothing.
  - `toggle_label_follows_state`.
- Note (as built): there is no `KeyBinding`. ⌃⌘S lives in the `on_key_down` keymap, after the modal shield. A menu key equivalent would fire before the window sees the key and bypass that shield, so the View menu cannot show ⌃⌘S. The shortcut is shown in the toolbar tooltip, the palette and the shortcut sheet. Like the other shortcuts, it is absorbed while a text field has focus.

### Task 5: Docs and gate (S)

- [ ] Update `docs/ux_audit.md` (sidebar), `docs/accessibility.md` (the new roles and labels, not VoiceOver-checked) and the shortcut sheet docs.
- [ ] Run the full gate: fmt, clippy, `cargo test --workspace`, `make ui-smoke`, and `make ui-e2e` (report SKIP honestly). Get an independent review per AGENTS §9.
- [ ] Before and after screenshots of the expanded and collapsed sidebar, from `make ui-smoke` or by hand. They live under `target/` and are not committed.

## Open questions for the owner

1. **Shortcut.** Should it be ⌃⌘S (the macOS standard, recommended) or something else?
2. **Animation.** Should the sidebar change instantly (this plan, S6), or with a short slide that respects Reduce Motion (more work, and a per-frame grid re-layout)?
3. **Rail contents.** Should the rail show only the default collections (this plan), or also one Smart Albums button and one Tags button that open a popover list?
4. **Icon set.** Is the icon table in S4 right? For example, should Screenshots use `camera` or `monitor`, and should Duplicates use `copy` or `layers`?
5. **Rail width.** Should it stay at 48 px (no grid layout change), or be a little wider (56 px) for easier clicking?

## Owner answers (2026-11-06)

1. Shortcut: ⌃⌘S.
2. Animation: instant (S6 as written).
3. Rail contents: default collections only (S5 as written).
4. Icon set: not changed; the S4 table stands.
5. Rail width: 48 px.
