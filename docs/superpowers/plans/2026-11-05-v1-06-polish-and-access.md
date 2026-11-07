# Polish and Access (0.7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** VoiceOver can reach and operate Lumina's main controls, the app ships a light and a system theme with an accent choice, one non-English locale proves the i18n catalog, and a user can save a diagnostics report and run a library health check themselves. There is no background network.

**Architecture:** Accessibility uses what GPUI 0.3.11 already provides (`.role`, `aria_*`, `aria_active_descendant`, `on_a11y_action`, synthetic children). A few `theme`-level helpers set role, label and tooltip together, so a control cannot get one without the others. The theme tokens are already functions (`theme::bg_app()`), so a runtime `Palette` swap changes every call site at once. i18n becomes a key/value table per locale, with completeness tests. Diagnostics is one plain-text report built from engine and app state off the UI thread, saved through a save dialog.

**Tech Stack:** Rust 2021, GPUI Kit 0.9 / gpui-pre 0.3.11 (re-exports `accesskit` 0.24.1, and `Role`, `Toggled`, `Orientation`, `AccessibleAction` = `accesskit::Action`, `gpui.rs:114`), rusqlite, rfd 0.15, serde_json, the existing ObjC bridge `src/player/macos_bridge.m` (only for the system language in C1). No new crates.

The plan has five parts the owner can approve independently: **A** accessibility, **B** appearance (theme, accent, motion, density), **C** localization, **D** diagnostics and health, **E** update check and shortcuts (recommendations only).

**Order:** B1 and C1 first (both small, no visible change). B1 must land before A5 (focus ring tokens) and B2. C1 must land before every task that adds user-facing text (A1–A4 labels, B2–B5, D2, E1), so new labels go straight through `t()`/`tf()` and C2 only moves the strings that exist today. Then A1–A6, B2–B5, C2–C3, D1–D2, E1.

## Global Constraints

- **Applies to every plan from now on, not only this one** (the index's "adopt its rules now"): new user-facing strings go through `i18n.rs` keys (`t`/`tf`/`tn` after C1; before C1, a `command.*`-style key in the current table), new colors through `theme.rs` tokens (never `rgb(0x…)`/`rgba(0x…)` in `app/` or `views/`), and every new interactive control gets `.id(...)`, a `.role(...)` and an `aria_label` (through the A1 helpers once they exist). B1's `no_color_literals_outside_theme` and C2's `no_new_hardcoded_ui_strings` enforce the first two; A1's helper signatures enforce the third. Review of plans 2–5 checks all three, so this sweep does not grow while they land.
- No new dependencies. Use `gpui::accesskit` (re-exported, `gpui-pre/src/gpui.rs:5`) for `Live`.
- No network in any task. Part E's recommended form makes no request from Lumina at all.
- Never claim VoiceOver support in docs, help or release notes until a human VoiceOver run of the A6 checklist has passed. Docs say "roles and labels added, VoiceOver check pending" until then (AGENTS §11, §15).
- Label building in the grid runs only when `window.is_a11y_active()` (`gpui-pre/src/window.rs:7273`). The grid render path must not get slower when no assistive technology is attached.
- **At most one element claims `aria_active_descendant` per frame.** GPUI panics in debug builds when two nodes claim it in one frame, or when the focused node claims it (`gpui-pre/src/window/a11y.rs:258–271, 513–527`). The grid cursor card claims it only when no overlay (palette, menu, modal) is open; the overlay's highlighted row claims it otherwise.
- **The focused element must itself be an a11y node.** GPUI honors an active descendant only when a focused ancestor is in the tree (`window/a11y.rs:269`, `focus_is_ancestor_of_current` :502), and a focused element without a role logs "has an id but no role" and is not reported (`a11y.rs:251–253`). Lumina's only focus target is the app root (`lumina-app-root`, `app/render/mod.rs:370–374`), so A1 gives it a role and label; without that, A2's and A4's active-descendant work does nothing.
- An AX "press" (`AccessibleAction::Click`) is synthesized by GPUI as a left mouse down/up at the node's center (`gpui-pre/src/window.rs:7323–7340`) and advertised automatically on every element with `on_click` (`div.rs:3655`). Do not add `on_a11y_action(Click)` where `on_click` exists; add it only to controls that react on `on_mouse_down` alone.
- Theme changes do not change pixels in the default (Dark + Amber) configuration. B1 is a pure refactor.
- Config gains `appearance`, `accent`, `motion`, `density` (all `#[serde(default)]`). Tests never persist config (`save_config` test early return).
- Diagnostics never includes search history, resume positions or catalog content (tags, captions, album or smart-album names, `last_view` tokens such as `tag:Family`, file names from catalog rows) beyond counts. Config goes into the report through an **allowlist**, so a field another plan adds is excluded until someone decides it is safe. The log tail (which does contain file paths and imported-video addresses) is included only after the user is told what it contains; URL query strings in it are cut (signed CDN links can carry tokens), and the home folder becomes `~` everywhere in the report.
- `#[core::prelude::v1::test]` in modules that glob-import `gpui::*`.
- debug_selectors (new, fixed; they follow the existing `preferences-*` names such as `preferences-tab-general`, `views/preferences.rs:31`): `preferences-tab-appearance`, `preferences-appearance-{system|light|dark}`, `preferences-accent-{amber|blue|green|graphite}`, `preferences-motion-{system|reduce|full}`, `preferences-density-{regular|compact}`, `preferences-health-check`, `preferences-health-rebuild-index`, `preferences-save-diagnostics`, `health-report`, `focus-region-{sidebar|grid|inspector}`. Sidebar rows reuse the existing `sidebar-nav-{label}` and `sidebar-tag-item-{tag}` (`views/sidebar/sections.rs:621`).
- Stage explicit paths. Never `git stash`. Never commit screenshots or `target/`.

## Review Focus

1. **Grid scroll performance with no screen reader attached.** Labels must not be built per frame. A2 test `grid_labels_skip_when_a11y_inactive`. `make bench` runs only `lumina-core`'s `benchmark_test` (`Makefile:73–74`) and does not measure rendering, so it is not evidence here; A6's checklist adds a scroll of a large library with VoiceOver on.
2. **Debug-build panic from two active-descendant claims** (grid card plus palette row in the same frame). A4 test `palette_open_over_grid_draws_without_panic` in a debug build. In-process it only proves the claim is gated in code, because a11y is never active headlessly; the live A6 probe opens the palette over the grid with AX active.
3. **A dark-only literal left behind makes light theme illegible.** B1 test `no_color_literals_outside_theme` and B2 `every_palette_meets_contrast` (text, muted, subtle, focus ring and accent fg on every surface, for every theme × accent).
4. **Theme switch leaves stale colors** (cached elements, native menus, file dialogs in the other appearance). B2 test `appearance_switch_repaints_with_new_palette` and a manual check of native menu and dialog appearance in A6's checklist.
5. **German strings overflow fixed-width controls** (toolbar, segmented controls, delete dialog buttons). C3 adds `LUMINA_LANG=de make ui-smoke` with a screenshot review. Clipping cannot be asserted in-process, so this is a manual review item.
6. **Diagnostics leaks private data.** D2 tests `report_omits_search_history_and_resume_positions`, `report_replaces_home_dir_everywhere`, `report_has_no_catalog_rows`, `report_config_is_allowlisted`, `log_tail_strips_url_query_strings`.
7. **The health check freezes the UI.** D2 test `health_check_never_waits_on_the_catalog_lock`, built on the existing `catalog_locks_while_held` helper (`tests/ui_thread.rs:26`). `quick_check(10)` caps the number of errors reported, not the time; it holds the catalog lock while it runs, which delays background work (scans) but not frames.
8. **Tab region focus traps keyboard users or eats Tab inside text fields and modals.** A5 tests `tab_cycles_regions_and_skips_hidden_inspector`, `tab_in_text_field_is_not_region_focus`, `tab_absorbed_by_modal`, `escape_precedence_in_sidebar_region`.

---

## Current state (verified 2026-11-05)

**Accessibility**
- There are no `.role(` or `aria_*` calls anywhere in `crates/lumina-ui-gpui/src` (grep). The only role is `Role::TextInput` passed to GPUI Kit's `InputAccessibility::new(Some(Role::TextInput)).with_label(...)` at `app/text_input.rs:153`.
- GPUI exposes these through `StatefulInteractiveElement` (`gpui-pre-0.3.11/src/elements/div.rs`): `role` :1357, `accessibility_id` :1373, `aria_label` :1379, `aria_description` :1388, `aria_keyshortcuts` :1398, `aria_active_descendant` :1418, `a11y_synthetic_children` :1432, `aria_selected/expanded/toggled` :1441–1454, numeric value/min/max/step :1459–1492, `aria_value` :1473, `aria_level/position_in_set/size_of_set` :1504–1516, `aria_row_index/column_index/row_count/column_count` :1522–1540, and `on_a11y_action` :1550. `focus_visible` :1342, `tab_group` :844, `FocusHandle::tab_index/tab_stop` (`window.rs:582,593`), `Window::focus_next/prev` (`window.rs:2652,2663`).
- **There is no `aria_live` setter.** A live region is possible through `a11y_synthetic_children(|b| b.parent_node().set_live(Live::Polite))` (`A11ySubtreeBuilder::parent_node`, `window/a11y.rs:364`; `Node::set_live` in accesskit 0.24.1).
- These are trait methods on `StatefulInteractiveElement` (`div.rs:1353`), so they need a `Stateful<Div>` (an element with `.id`). `UniformList` implements only `InteractiveElement` (`elements/uniform_list.rs:717`), so the list itself cannot take a role; the grid role goes on its wrapper `media-grid-surface` (`app/render/grid.rs:69`).
- A node only reaches the tree when it has an id and a role other than `GenericContainer` (`div.rs:1955–1961`). `debug_selector` is not an a11y id. A focused element with an id but no role is not reported either (`window/a11y.rs:251–253`), and active descendants are honored only under a focused node (`a11y.rs:269`). Two claims in one frame, or a claim by the focused node itself, panic in debug builds (`a11y.rs:258–268, 513–524`).
- `on_click` advertises `Action::Click` automatically (`div.rs:3655`), and GPUI performs it as a synthetic left mouse down/up at the node's center (`window.rs:7323–7340`).
- **The headless test platform never activates a11y.** `PlatformWindow::a11y_init` defaults to a no-op (`gpui-pre/src/platform.rs:1441`), the test platform (`gpui-pre/src/platform/test/`) does not override it, and the active flag is set only by the platform activation callback (`window.rs:1732–1760`). `TestAppContext` can only *disable* accessibility (`app/test_context.rs:196`); nothing enables it. So `Window::is_a11y_active()` is always false and `Window::debug_a11y_tree_json()` (`window.rs:7278`) returns nothing in `VisualTestContext`. **In-process tests cannot inspect the a11y tree or exercise any `is_a11y_active()` branch.** Real verification needs a live window plus an AX client (A6) and a human VoiceOver run.
- Lumina has one app-level `FocusHandle` (`app/state.rs:319`, tracked at `app/render/mod.rs:309,334,374`), plus one for the floating inspector (`views/floating_inspector.rs:11`). Only the main tree's root has an id (`lumina-app-root`, `app/render/mod.rs:371`); none has a role. The welcome and first-scan roots (`:305–309`, `:330–334`) have neither. Keys are hand-dispatched in `app/keymap/mod.rs:10` (`handle_key_down`). Tab is handled only inside the smart-album draft (`app/keymap/modals.rs:416,427`). The sidebar has no keyboard navigation; its destinations are reachable through the palette's GO TO group.
- Icon buttons already share `icon_button(id, on)` (`app/render/toolbar.rs:11`), and `window_button` (`toolbar.rs:52`) wraps it with a tooltip. Other tooltips are attached at each call site through `theme::tooltip_label` (`theme.rs:382`; toolbar.rs:55,132,228,237,246,301). That gives one place to attach roles and labels.
- The grid is `uniform_list("media-grid", total_rows, …)` (`app/render/grid.rs:123`) inside `media-grid-surface` (`:69`). Rows have `.id(row_ix)` (`:583`) and cards `.id(item_id_num)` (`:238`), so card identity is the media id, as AccessKit needs across frames. Cards use `on_click` (`:267`). Only visible rows exist, which is the virtualization limit for AT.
- Reduce Motion exists: `motion::reduce_motion()` reads `NSWorkspace` once through the `OnceLock` (`motion.rs:33–42`, bridge `macos_bridge.m:546`). It never calls GPUI's `cx.set_reduce_motion` (`gpui-pre/src/app.rs:1411`). A relaunch is needed after the system setting changes (`docs/accessibility.md:27–28`). `Context::observe_window_activation` exists (`gpui-pre/src/app/context.rs:410`).

**Theme**
- `theme.rs` (509 lines) defines 45 color tokens as `fn() -> Rgba` through the `tokens!` macro (`theme.rs:23–96`), plus `scrim` (:153), `modal_dim` (:160), `selection_wash` (:176, amber at 12%, `0xF0B54B1F`), `card_surface` (:186) and control shapes.
- Through-token call sites: 518 `.bg/.text_color/.border_color(theme::x())` uses, and 1,033 `theme::` references in total (non-test code, grep). **Hard-coded color literals outside theme.rs: 13 `rgb(0x…)`/`rgba(0x…)`, in 6 files**: `app/render/grid.rs:466,475,484,509,530,537,541` (white on a dark scrim over photos), `app/render/rail.rs:20` (`0x222326`, matches no token) and `:40`, `app/render/toolbar.rs:13`, `views/inspector/body.rs:521`, `views/album_builder/rows.rs:209` (transparent) and **`views/viewer/transport.rs:267` (`rgba(0xF0B54B38)`, the accent at 22%, a different alpha from `selection_wash`)**. Two more use GPUI's `transparent_black()` (`views/album_builder/mod.rs:86,88`). A second theme is therefore cheap.
- Contrast tests already exist (`theme.rs:432–509`: WCAG luminance; focus ring ≥3:1 on 6 surfaces; `text` and `text_muted` ≥4.5:1, `text_subtle` ≥4.0:1 and `rating_unset` ≥3:1 on 7 surfaces; plus accent_fg/accent, selection_fg/selection_bg, danger_text/bg_app). `text_subtle` is deliberately below AA's 4.5:1, so docs must not claim AA for hint text. They cover only the one palette.
- No window or app appearance is set anywhere, so native menus and dialogs follow the system appearance while the GPUI content is always dark. GPUI can read appearance (`Window::appearance`, `window.rs:3144`; `observe_window_appearance`, :2534) **and set it: `App::set_window_appearance(Option<WindowAppearance>)` (`gpui-pre/src/app.rs:1831`) sets `NSApplication.appearance` on macOS (`gpui-pre-macos/src/platform.rs:785`); `None` follows the system again.** No ObjC bridge function is needed.
- Config (`config.rs:319`) has no appearance, accent, motion or density fields. Preferences tabs are General, Storage and About (`app/preferences.rs:10`, `views/preferences.rs:28–42`), with selectors `preferences-tab-{general|storage|about}`.
- About 40 fixed `h(px(20–32))` heights in app/views/theme (39 by grep). Rows are `theme::row` at 26px (`theme.rs:250–253`) and controls are 28px (`theme.rs` `control_base`).

**i18n**
- `i18n.rs` (94 lines) has 51 keys, all `command.*`. `t()` re-reads `LUMINA_LANG` on every call (`i18n.rs:13–25`). The only caller is `Command::title` (`app/command_palette.rs:72–74`).
- **Strings that bypass the catalog (heuristic grep, non-test code in `app/`, `views/`, `menus.rs`, `actions.rs`):** about 467 capitalized multi-word literals and about 205 capitalized single words, so **roughly 650 user-facing strings**. The figure includes some `format!` templates and over-counts by an unknown margin. The heaviest files are `app/render/mod.rs` (51), `app/render/grid.rs` (29), `app/navigation.rs` (25) and `menus.rs` (18; native menu titles are all literals, `menus.rs:7–40`, and duplicate the palette's catalog labels).
- English plural hacks: 6 sites, `app/import.rs:232` (`"item"`/`"items"`), `app/library.rs:1224,1263`, `views/preferences.rs:239`, `views/duplicates.rs:87`, `views/tag_manager/detail.rs:38`. Month names are hard-coded at `app/types.rs:176` (`month_name`, used by `grid.rs:1012` and `viewer/top_bar.rs:53`). **chrono `%b`/`%e` formats print English month abbreviations** at `views/network_import_dialog.rs:16`, `views/inspector/batch.rs:390–394` and `views/inspector/tabs/info.rs:300`. `format_bytes` (`app/types.rs:147`) uses `.` as the decimal separator.
- Native menus are built once at startup (`main.rs:45`, `cx.set_menus(build_menus())`), so a locale resolved once per launch is enough for them.
- `Info.plist` has `CFBundleDevelopmentRegion=en` and no `CFBundleLocalizations`, and the bundle has no `.lproj` folders, so AppKit-supplied text (open and save panels, any system menu items) stays English whatever `t()` returns. Error text from `lumina-core` (for example "Library folder is not available") reaches the banner in English.

**Diagnostics and health**
- `Catalog::quick_check` (`catalog/mod.rs:200`, `PRAGMA quick_check(1)`) runs at open, and the result is only logged (`lumina-core/src/lib.rs:292–295`), so the user never sees it. A daily backup exists (`catalog/mod.rs:212`). The search index is contentless FTS5 (`schema.rs:179–193`, `content=''`), so a plain `DELETE FROM media_search_fts` is rejected; rows leave only through the `'delete'`/`'delete-all'` commands. A full rebuild statement already exists inline in a migration (`schema.rs:501–512`). The catalog runs in WAL mode with `busy_timeout = 5000` (`schema.rs:13–14`). Missing-file ids come from `Catalog::get_missing_ids` (`catalog/files.rs:350`).
- The UI thread never takes the catalog lock while drawing; `tests/ui_thread.rs` proves this with `catalog_locks_while_held` (:26) and `LuminaEngine::catalog_locks_on_this_thread`.
- Logs live at `logging.rs:55` `log_file_path()` and rotate at 5 MB (`logging.rs:77`). Preferences can reveal the logs, config and cache folders (`app/preferences.rs:156–181`) through a private `reveal()` that runs `open` and returns early under `cfg!(test)` (`app/preferences.rs:84–92`). Logs contain file paths and the full pasted download URLs, query string included (for example `app/downloads/job.rs:69`).
- There are no secrets in `AppConfig` today. The private-but-not-secret fields are `recent_searches`, `last_search_query`, `resume_positions`, `last_library_path`, `last_selected_media_id`, `external_editor` and **`last_view`, whose token is `tag:{tag name}` for a tag view (`config.rs:32–43`)**.

**Update check and shortcuts**
- No update code exists. Version 0.2.0 is in `resources/Info.plist:22`. No release feed URL is defined anywhere in the repo, and the repo location and visibility are still open in plan 1.
- Shortcuts are hard-coded matches across `app/keymap/{mod,grid,viewer,modals}.rs` (997 lines, 0 `KeyBinding` tables). The palette shortcut hints are `&'static str` (`command_palette.rs:67`).

## Decisions

1. **Use GPUI's AccessKit API in place, with no bridge layer.** Rejected: an upstream GPUI change or a custom NSAccessibility layer, which the roadmap §10 suggests. That is stale, because GPUI 0.3.11 already ships a complete AccessKit path.
2. **Role, label and tooltip are set by one helper** (`icon_button(id, label, on)`, `theme::a11y_button`). The signature forces a label. Rejected: adding `.role().aria_label()` at each call site, which is easy to forget and impossible to test in-process.
3. **Grid = `Role::Grid` on `media-grid-surface` with total `aria_row_count`, visible rows = `Role::Row`, visible cards = `Role::GridCell`, and the keyboard-selected card marks itself `aria_active_descendant`.** This works only because the focused app root becomes an a11y node (A1); VoiceOver should then follow arrow-key selection and the existing scroll-into-view, which A6 confirms by hand. Rejected: synthetic nodes for every item (250k nodes per frame) and `ListBox` (loses 2-D position).
4. **Keyboard focus by region, not per control.** Tab and Shift+Tab move between Sidebar, Grid and Inspector. The region gets a focus ring, and arrows act inside it. Rejected: a `FocusHandle` per control, which means rewriting the hand-dispatched keymap. Toolbar buttons stay reachable through the palette and menus (§11 keyboard path), and VoiceOver can still press them, because `on_click` already advertises the AX press action.
5. **Verification = live AX dump plus a human VoiceOver checklist.** In-process tests cover label text, keyboard focus and absorption, because the test platform cannot activate a11y. Rejected: patching the vendored gpui-pre test window, which is registry code and would be lost on update.
6. **Palette swap through a thread-local `Cell<&'static Palette>` read by the existing token fns.** All 518 call sites stay unchanged. Native chrome follows through GPUI's own `cx.set_window_appearance` (Light/Dark forced, `None` for System), not a new ObjC call. Rejected: threading a `Theme` through `cx` globals, which touches every render function.
7. **The viewer stays dark in the light theme** (`bg_viewer`, `bg_media`, viewer HUD), the convention of photo apps like Lightroom. Only the chrome changes. Rejected: a light viewer, which hurts photo judgement and doubles the HUD contrast work. Listed as an owner question.
8. **Accent = 4 hand-picked presets with fixed hex values per theme**, not arbitrary color or system accent. Every preset can be contrast-tested. Rejected: deriving shades from `NSColor.controlAccentColor`, which cannot be tested and can fail contrast.
9. **Locale = German (`de`).** It has long compounds, so it stresses layout, uses Latin script and LTR, and has the same `one/other` plural rule as English, so no CLDR crate is needed. Rejected: French (0 counts as singular, needs a rule), Spanish (shorter strings, weaker layout proof), RTL (needs a layout audit first). This is an owner question.
10. **Catalog = static `&[(&str, &str)]` tables per locale, `{name}` placeholders, and `tn()` with `.one/.other` keys.** Dates use a catalog month table plus a per-locale pattern key (`date.day_month_year` = `"{month} {d}, {y}"` / `"{d}. {month} {y}"`); the decimal separator in `format_bytes` comes from one catalog key. Grouping separators in counts and other number formats stay en-US (Out of scope). Rejected: Fluent or gettext crates (a new dependency, more than two locales need), and chrono's `unstable-locales` feature (pulls `pure-rust-locales`, a new dependency in effect).
11. **Diagnostics = one plain-text `.txt`** chosen with a save dialog. The user can read it before sending. Config enters it through an allowlist, not a denylist. Rejected: a zip bundle (needs a zip crate or `ditto`, and the user cannot inspect it).
12. **Update check: recommend Help > "Check for Updates…" that opens the releases web page in the default browser.** Lumina makes no request. Rejected for now: fetching a GitHub releases JSON or an appcast with system curl. That needs a public feed URL, version parsing and a privacy note, and the real update mechanism is on the roadmap for 1.0.
13. **Custom shortcuts are deferred.** The keymap is code, not data (997 lines, no binding table), so remapping first needs a keymap-as-actions refactor (L).
14. **Density changes only list and control heights** (rows 26→22, controls 28→24). Grid density is already the zoom and fit setting. This is an owner question; it can be dropped.

---

## Part A: Accessibility

### Task A1: a11y helpers + toolbar, window controls, sidebar (S/M)

**Files:**
- Create: `crates/lumina-ui-gpui/src/a11y.rs` (label builders, no GPUI types beyond `Role`)
- Modify: `crates/lumina-ui-gpui/src/theme.rs` (`a11y_button`), `app/render/toolbar.rs` (`icon_button`, `window_button`), `app/render/mod.rs` (app root role), `app/render/rail.rs`, `views/sidebar/{mod,sections}.rs`, `views/search_bar.rs`, `main.rs` (mod), `i18n` (label keys)

**Interfaces:**
- App root `lumina-app-root` (`app/render/mod.rs:371`): `.role(Role::Application)` (or `Role::Group`, whichever VoiceOver announces less noisily in A6) with `aria_label` = window title. It holds the app `FocusHandle`, so this is what lets active descendants (A2, A4) work at all; the welcome and first-scan roots get the same id, role and label.
- `fn icon_button(id: &'static str, label: impl Into<SharedString>, on: bool) -> Stateful<Div>` sets `.role(Role::Button).aria_label(label).tooltip(tooltip_label(label))`; the call-site `.tooltip(...)` calls it replaces are removed so each button has one tooltip. Toggles use `aria_toggled(Toggled::True/False)` (sidebar, inspector, downloads toggles).
- `pub fn theme::a11y_button(el: Stateful<Div>, label) -> Stateful<Div>` for text buttons.
- Toolbar container `Role::Toolbar`, label "Toolbar". Sidebar container `Role::Navigation` containing a `Role::Tree`; rows `Role::TreeItem` (a TreeItem needs a Tree parent) with `aria_selected(active)`, `aria_level` (tag nesting), `aria_expanded` on collapsible section headers, and `aria_label` "{name}, {count} items".
- `a11y::sidebar_row_label(name: &str, count: Option<usize>) -> String`, built with `tn("a11y.items", count)` so it is localized from the start.

- [ ] Tests (unit, `a11y.rs`): `sidebar_row_label_includes_count`, `sidebar_row_label_without_count`, `sidebar_row_label_uses_singular_for_one`. They would catch an unlabeled or garbled row.
- [ ] Compile-time enforcement: every `icon_button` caller passes a label. A missing label fails the build. The in-process tests cannot see roles (a11y never activates headlessly), so role wiring is checked by A6's live probe, not here.
- [ ] Run fmt, clippy, `cargo test -p lumina-ui-gpui`. The existing toolbar and sidebar automation tests must still pass (behavior unchanged).
- [ ] Docs: `docs/accessibility.md` "Screen readers" lists the areas with roles and says "VoiceOver check pending".

### Task A2: grid cards and active descendant (M)

**Files:** `app/render/grid.rs`, `a11y.rs`

**Interfaces:**
- Grid container `media-grid-surface` (`grid.rs:69`; `uniform_list` cannot take a role): `Role::Grid`, `aria_row_count(total_rows)`, `aria_column_count(cols)`, `aria_label` = current view title. Rows (`grid.rs:583`): `Role::Row`, `aria_row_index`.
- Card: `Role::GridCell`, `aria_row_index`, `aria_column_index`, `aria_selected`. When it is the keyboard cursor item **and no overlay is open**, `aria_active_descendant()` (one claimer per frame, Global Constraints). No `on_a11y_action(Click)`: the card's `on_click` (`grid.rs:267`) already provides the AX press, delivered as a synthetic click at the card's center.
- `a11y::card_label(name: &str, kind: MediaKind, date: Option<NaiveDate>, rating: u8, favorite: bool, missing: bool) -> String` gives, for example, "IMG_0042.jpg, video, 3 March 2024, 4 stars, favorite". Words and the date go through the catalog (C1).
- Labels are built only under `if window.is_a11y_active()`, and roles and ids are set unconditionally (cheap).

- [ ] Tests (unit): `card_label_orders_fields_and_skips_empty`, `card_label_marks_missing_file`. UI automation (`tests/ui_automation.rs`): `grid_labels_skip_when_a11y_inactive`. It checks a test-only label-build counter that stays 0 after rendering 200 items, which would catch the gate being removed. It cannot prove labels are built when a screen reader *is* attached (the branch never runs headlessly); A6's probe checks that a cell has a non-empty description.
- [ ] No render benchmark exists (`make bench` is core-only), so do not cite it. Record in the commit body that the gate test is the evidence, and add "scroll a 10k-item library with VoiceOver on; no visible stutter" to the A6 checklist.
- [ ] Keyboard: unchanged (arrows, Enter, Space). VoiceOver follows through active descendant (A6 checks it).

### Task A3: viewer and video transport (M)

**Files:** `views/viewer/{top_bar,transport,stage_controls,viewport}.rs`, `a11y.rs`

**Interfaces:**
- `viewer-viewport`: `Role::Image`, with `aria_label` the same as `card_label` plus "{n} of {total}".
- `viewer-back`, `video-play`, `video-speed` and `viewer-zoom-fit`: buttons through `icon_button`/`a11y_button`.
- `video-scrubber`: `Role::Slider`, numeric value, min 0, max duration, step 5 s, `aria_value("1:23 of 4:56")` (through `tf`). `on_a11y_action(AccessibleAction::Increment/Decrement)` → one named fn `LuminaApp::seek_by_step(dir)` that the ←/→ key path also calls.
- Viewer position text `viewer-position`: `Role::Label`.

- [ ] Tests (unit): `time_value_text_formats_hours`. UI automation (`tests/video_scrub.rs`): `scrubber_step_forward_and_back_moves_player`. It calls `seek_by_step` and asserts the player position for both directions. It cannot invoke the a11y listener itself (no a11y headlessly); the listener is a one-line call to `seek_by_step`, and A6's checklist has "VoiceOver: increment the scrubber".

### Task A4: modals, palette, menus, toasts, error banner (M)

**Files:** `views/{delete_modal,import_dialog,network_import_dialog,downloads_confirm,smart_album_delete,command_palette,context_menu,sort_menu,toast,preferences}.rs`, `app/render/mod.rs` (banner)

**Interfaces:**
- Confirmations: `Role::AlertDialog`. Other overlays: `Role::Dialog`. Each has a label equal to its title.
- Palette: `Role::ListBox`, rows `ListBoxOption` + `aria_selected` + `aria_active_descendant` on the highlighted row (the grid card stops claiming while the palette is open), and `aria_keyshortcuts(shortcut)`. Same rule for the highlighted row of context and sort menus.
- Context and sort menus: `Role::Menu`/`MenuItem`. Preferences tabs: `TabList`/`Tab` + `aria_selected`.
- Toast: `Role::Status` + `Live::Polite` through `a11y_synthetic_children(|b| b.parent_node().set_live(accesskit::Live::Polite))`. Error banner: `Role::Alert` + `Live::Assertive`. Give both a stable id so a new message is a value change on the same node, not a new node each time.

- [ ] Tests: existing overlay-isolation and modal key-absorption tests still pass. Add unit `palette_row_label_includes_shortcut`, and UI `palette_open_over_grid_draws_without_panic` (debug build, palette opened while a grid card is the cursor). Headlessly that only exercises the non-a11y path; the claim gate itself is a plain `bool` check reviewed in code and exercised by A6's live probe. Live-region behavior is checked only in A6, and the plan says so.

### Task A5: region focus and visible focus rings (M)

**Files:** `app/state.rs` (small `FocusRegion` struct, not flat fields), `app/keymap/mod.rs`, `app/navigation.rs` (sidebar cursor), `app/render/{mod,grid}.rs`, `views/sidebar/sections.rs`, `views/inspector/mod.rs`

**Interfaces:**
- `pub enum FocusRegion { Sidebar, Grid, Inspector }`. `LuminaApp::cycle_focus_region(&mut self, reverse: bool, cx)` skips hidden panes.
- Tab and ⇧Tab when no modal or text field owns the key. In Sidebar, ↑/↓ move a sidebar cursor and ↵ opens the destination. Escape returns to Grid, but only after the existing Escape consumers (toast, then error banner, `docs/accessibility.md` "Errors and Trash") have had it.
- The region is app state only; real focus stays on the app root (so AX focus does not move). Each region container gets `Role::Group` + label so VoiceOver can name where the ring is.
- The focused region draws a 2px `theme::focus_ring()` inset on its cursor row or card. The grid already uses selection styling. The ring is neutral, as `theme.rs:46` requires.
- Selectors: `focus-region-sidebar|grid|inspector`.

- [ ] Tests (UI automation, `tests/interaction.rs`): `tab_cycles_regions_and_skips_hidden_inspector`, `tab_in_text_field_is_not_region_focus`, `tab_absorbed_by_modal`, `sidebar_arrows_and_enter_open_destination` (asserts the active view changed, and would catch arrows leaking to the grid), `escape_precedence_in_sidebar_region` (with a toast showing, the first Escape dismisses the toast and the region stays Sidebar; the second returns to Grid).
- [ ] Shortcut sheet (`⌘/`) and `docs/accessibility.md` "Keyboard access" document Tab regions.

### Task A6: live AX verification + VoiceOver checklist (S, plus a human run)

**Files:** `scripts/live_ui_automation.sh` (an `ax_dump.swift` helper beside the existing CG helpers), `docs/accessibility.md`, `RELEASING.md` (accessibility checklist line)

**Interfaces:**
- `ax_dump.swift <pid>` (a heredoc in the script, like the existing `window_probe.swift`/`key_post.swift` helpers at `live_ui_automation.sh:39–102`) walks `AXUIElementCopyAttributeValue(kAXChildrenAttribute)` and prints role, description and title. The first query makes AccessKit activate; GPUI then forces a redraw before the full tree exists, so the helper re-queries for up to 5 s before failing.
- Script checks: an `AXButton` whose description starts with "Hide Sidebar" or "Show Sidebar" (it reads "Hide Sidebar (⌃⌘S)" while the sidebar is shown), an `AXToolbar`, an `AXGrid` with ≥1 cell whose description is non-empty, and a text field labeled "Search". Then it posts ⌘K and checks the window still responds (the active-descendant panic would kill a debug build). Without Accessibility permission it reports **SKIP**, not pass (AGENTS §6.4).

- [ ] Manual VoiceOver checklist in `docs/accessibility.md`, covering the ROADMAP §10 acceptance flow (import/open a library, search, select, tag, view, export without a mouse, no keyboard trap): open a library, import a folder, search, Tab between regions, move selection with arrows (VoiceOver reads the card), open the viewer, play and scrub video (including VoiceOver increment on the scrubber), tag in the inspector, export, delete with a confirmation, toast and error banner announced, palette open over the grid, scroll a 10k library with VoiceOver on, theme switch with native menu and dialog appearance. **A human must run it**, and the result and date are recorded in the doc.
- [ ] Only after a passing human run, update AGENTS.md §11/§15 and `docs/accessibility.md` to describe the verified scope. Until then they keep "not yet checked".

---

## Part B: Appearance

### Task B1: `Palette` refactor, no visual change (M)

**Files:** `crates/lumina-ui-gpui/src/theme.rs`, plus the 6 files with literals listed in Current state

**Interfaces:**
- `tokens!` generates `pub struct Palette { pub bg_media: Rgba, … }` and `pub const DARK: Palette`. Each token fn becomes `pub fn bg_app() -> Rgba { active().bg_app }`.
- `thread_local! { static ACTIVE: Cell<&'static Palette> }`, `pub fn set_palette(&'static Palette)`, `pub fn active() -> &'static Palette`.
- New tokens replace the 13 literals: `on_media_scrim` (0x00000099), `on_media_text` (white), `transparent`, `rail_border` (0x222326), `accent_wash` (= today's `selection_wash`, 12%) and `accent_wash_strong` (22%, replaces `transport.rs:267`). Two wash tokens, because merging them would change the played-bar pixels and break this task's no-visual-change rule. `scrim`, `modal_dim`, `selection_wash` and `card_surface` move into `Palette` too.

- [ ] Tests (unit, theme.rs): `dark_palette_matches_v3_sheet` pins every DARK value (the 45 tokens plus the moved ones), so the refactor stays pixel-identical; 6 samples would let the other 39 drift. `no_color_literals_outside_theme` scans `src/**/*.rs` (from `env!("CARGO_MANIFEST_DIR")`, via `std::fs` at test time, skipping `src/tests/`) for `rgb(0x`, `rgba(0x`, `hsla(`, `white()` and `black()` outside theme.rs, which would catch new dark-only literals; `transparent_black()` stays allowed. The existing contrast tests now loop over `ALL_PALETTES`.
- [ ] `make ui-smoke`, then compare the screenshot by eye with the one from before.

### Task B2: Light palette + Appearance setting (M)

**Files:** `theme.rs` (`LIGHT`; theme.rs is already 509 lines, so palettes go to a new `theme/palettes.rs` or `appearance.rs` beside it), `config.rs` (`appearance: Appearance`), `app/preferences.rs` + `views/preferences.rs` (new **Appearance** tab), `app/command_palette.rs`, `i18n`, `main.rs` (observe appearance; apply the saved choice before the first window opens)

**Interfaces:**
- `pub enum Appearance { System, Light, Dark }` (serde lowercase). Default is an owner question (System or keep Dark); until answered, implement `Dark` as the default so existing users see no change.
- `pub fn resolve_palette(a: Appearance, w: WindowAppearance) -> &'static Palette` (`Light`/`VibrantLight` → LIGHT, `Dark`/`VibrantDark` → DARK).
- Native chrome: `cx.set_window_appearance(Some(Light|Dark))` for a forced choice, `None` for System (`gpui-pre/src/app.rs:1831`). GPUI notes that a forced appearance stops windows from tracking system changes, which is the intent.
- `window.observe_window_appearance` (System mode only) → `set_palette` + `cx.refresh_windows()`. Viewer tokens stay dark in `LIGHT` (Decision 7).
- Palette commands: "Appearance: System / Light / Dark". Selectors `preferences-tab-appearance`, `preferences-appearance-*`.

- [ ] Tests: unit `resolve_palette_follows_system_only_in_system_mode`, `every_palette_meets_contrast` (B1 loop now includes LIGHT; adds the light-specific `selection_fg` on `selection_bg` pair; thresholds stay as today, `text_subtle` 4.0), `light_viewer_tokens_stay_dark`. UI automation (`tests/preferences.rs`): `appearance_switch_repaints_with_new_palette` clicks `preferences-appearance-light` and asserts `config.appearance == Light` and `theme::bg_app() == LIGHT.bg_app` (the test resets the thread-local palette to DARK on exit). Keyboard: `palette_sets_appearance`. Native appearance cannot be read back from the test platform; A6's checklist covers menus and dialogs.
- [ ] Docs: README/help "Appearance". `docs/accessibility.md` contrast note.

### Task B3: Accent presets (S)

**Files:** `theme.rs` (`Accent { Amber, Blue, Green, Graphite }`, `AccentTokens` per theme), `config.rs`, preferences Appearance tab, palette

**Interfaces:** `pub fn set_accent(Accent)`. The accent, accent_hover, accent_fg, selection_*, `accent_wash` and `rating` tokens read the active accent. Label colors (`label_*`) never change. The theme.rs header rule becomes "the accent always means selection".

- [ ] Tests: `every_palette_meets_contrast` iterates theme × accent (8 combinations). `accent_never_equals_focus_ring` (focus stays neutral). UI: `accent_choice_persists_in_config`.

### Task B4: Live system Reduce Motion, optional in-app override (S)

**Files:** `motion.rs`, `app/mod.rs` (window activation observer); with the override also `config.rs` and the Appearance tab

**Interfaces:**
- Required (fixes the relaunch limitation): the cached system value moves from the `OnceLock` to an atomic refreshed on window activation (`cx.observe_window_activation`, `app/context.rs:410`), and every refresh calls `cx.set_reduce_motion(..)` so GPUI's own animations follow.
- Optional, owner question 9: `pub enum MotionPref { System, Reduce, Full }`, with `motion::reduce_motion()` = pref override, else the system value. macOS already has the system switch, so ship the override only if the owner wants it (ROADMAP lists "motion settings" for 0.7).

- [ ] Tests (unit, motion.rs): `system_value_refresh_is_seen_by_reduce_motion` (through the existing `#[cfg(test)]` seam, `motion.rs:33–37`). With the override: `reduce_pref_overrides_system_off`, `full_pref_overrides_system_on`. The existing `reduce_motion_keeps_fades_but_drops_rise_and_pop` stays.
- [ ] Docs: `docs/accessibility.md` "Motion": replace "relaunch after changing it" with "takes effect when you switch back to Lumina".

### Task B5: Density (S; owner may drop)

**Files:** `theme.rs` (`row_height()`, `control_height()`), `config.rs`, the Appearance tab, call sites among the 40 fixed heights that are list rows or controls (not icons or thumbnails)

- [ ] Tests: unit `compact_heights_stay_at_least_22px` (target size), UI `density_compact_shrinks_sidebar_rows` (asserts rendered bounds of `sidebar-nav-*`).

---

## Part C: Localization

### Task C1: Catalog tables, placeholders, plurals, system locale (S)

**Files:** `crates/lumina-ui-gpui/src/i18n.rs` → `i18n/{mod,en_us,de}.rs`, `player/macos_bridge.m` (`lumina_preferred_language()` from `NSLocale.preferredLanguages[0]`), `resources/Info.plist` (`CFBundleLocalizations` = en, de), the bundle step `scripts/build_release.sh:51–57` (empty `Contents/Resources/{en,de}.lproj` folders, so AppKit picks German for its own panels and menu items when the system language is German)

**Interfaces:**
- `pub static EN_US: &[(&str, &str)]`, `pub static DE: &[(&str, &str)]`. A `DE` table may be empty until C3; the completeness test is `#[ignore]`d with a reason until C3 fills it, and C3 removes the ignore.
- `pub fn t(key) -> String`, `pub fn tf(key, args: &[(&str, &dyn Display)]) -> String` (`{name}` replace), `pub fn tn(key, n: usize) -> String` (`key.one`/`key.other`, `{n}`). Each has a pure `*_in(locale, …)` twin so tests pick a locale without touching the process-wide one.
- The locale is resolved once (`OnceLock`): `LUMINA_LANG` first, then the system language matched by prefix (`de-DE`, `de-CH` → `de`), then en-US. A missing key falls back to English, then to the key, and logs the key once with `log::warn!`.

- [ ] Tests (unit): `de_covers_every_en_key` lists any missing keys in the failure message (**the untranslated-key test**). `de_has_no_extra_keys`. `no_duplicate_keys_in_a_table` (a static slice allows duplicates; the second would silently lose). `placeholders_match_between_locales` (same `{…}` set per key). `plural_keys_come_in_pairs`. `missing_key_falls_back_to_english_then_key`. `system_language_prefix_matches`. `en_values_have_no_trailing_whitespace`. `every_command_key_exists` (each `Command.key` in `command_palette.rs` is in `EN_US`, so a typo cannot show a raw key).

### Task C2: Move strings into the catalog (L, split into 3 commits)

**Files:** C2a `menus.rs`, toolbar, sidebar, preferences, shortcut sheet (A1–A4 labels already use keys). C2b dialogs, toasts, error banner prefixes, the 6 plural sites, month names (`app/types.rs:176`), the chrono `%b`/`%e` date sites (`network_import_dialog.rs:16`, `inspector/batch.rs:390–394`, `inspector/tabs/info.rs:300`, through the month table and `date.*` pattern keys) and the `format_bytes` decimal separator. C2c inspector, viewer, onboarding and the remaining files.

**Interfaces:** Native menu items use the same `command.*` keys as the palette, so there is one source. Menu-only titles (`Menu::new("File")`, "Quit Lumina") get `menu.*` keys.

- [ ] Test: `no_new_hardcoded_ui_strings`, a ratchet test that counts capitalized string literals passed to `.child(` / `tooltip_label(` / `set_error(` / `show_toast(` / `MenuItem::action(` / `Menu::new(` under `src/{app,views}` and `menus.rs` and asserts ≤ a baseline constant. **It lands in C2a's first commit**, before plans 2–5 add more UI, so the baseline is fixed early; each later commit lowers it, and new literals fail CI. It is a heuristic and is documented as such in the test doc comment. The ~650 figure in Current state is a sizing estimate only; the ratchet's own count is the number C2 works against.
- [ ] Test: `menus_use_catalog_titles` (every `MenuItem` title in `build_menus()` equals `t(key)` for some key, so menu and palette cannot drift).
- [ ] Test: `de_dates_use_german_month_names` (`date_label_in(Locale::De, 2024-03-03)` = "3. März 2024").
- [ ] Out of reach here: `lumina-core` error strings stay English (listed under Out of scope).

### Task C3: German translation + layout pass (M, needs a reviewer fluent in German)

**Files:** `i18n/de.rs`, `docs/accessibility.md` (Localization), README

- [ ] Translate every key. Keep query syntax (`tag:`, `rating:`) and product names untranslated.
- [ ] Remove the `#[ignore]` from `de_covers_every_en_key`.
- [ ] `LUMINA_LANG=de make ui-smoke`, then review the screenshot for clipping in the toolbar, segmented controls, delete dialog and preferences. Screenshot capture needs Screen Recording; if it is unavailable, say so and do the review by hand in a `LUMINA_LANG=de make dev` window. Fix by allowing wrap or flexible widths, never by shortening the German text to fit. Also check that open and save panels are German with the system language set to German (the `.lproj` folders from C1; `LUMINA_LANG` alone does not change AppKit's panels).
- [ ] A native speaker reviews the strings before release (owner question: who).

---

## Part D: Diagnostics and library health

### Task D1: Core health report + search index rebuild (M)

**Files:** `crates/lumina-core/src/catalog/health.rs` (new, small), `catalog/schema.rs` (extract `rebuild_search_index_in_tx` from `:501–512`, used by the migration and repair), `catalog/mod.rs`, `lib.rs`

**Interfaces:**
- `pub struct HealthReport { pub schema_version: i64, pub integrity: Result<(), String>, pub search_index: Result<(), String>, pub media_count: usize, pub missing_count: usize, pub indexed_count: usize, pub last_backup: Option<PathBuf> }`
- `Catalog::health_report(&self) -> Result<HealthReport>` runs `PRAGMA quick_check(10)` and `INSERT INTO media_search_fts(media_search_fts) VALUES('integrity-check')`, and compares the fts row count with the media count. On a contentless table `integrity-check` verifies only the index's internal structure, not that it matches `media_items`; the row-count comparison is the drift check, and it cannot see a row whose terms are stale. Say both in the doc comment. Count with `SELECT count(*) FROM media_search_fts_docsize` if a full scan of the contentless table turns out not to count rows (check while implementing).
- `Catalog::rebuild_search_index(&self) -> Result<usize>` in one transaction, through the extracted fn.
- `LuminaEngine::health_report()` and `rebuild_search_index()` are called from a background thread by the UI. Both hold the catalog lock while they run (quick_check reads the whole file), which delays scans and imports but never a frame, because the UI thread does not take the lock.

- [ ] Tests (`tests/core_tests.rs`): `health_report_clean_catalog_is_ok`, `health_report_detects_fts_drift` (empty the index with `INSERT INTO media_search_fts(media_search_fts) VALUES('delete-all')`, since a contentless table rejects `DELETE FROM`; then expect `indexed_count < media_count` and a non-OK `search_index`), `rebuild_search_index_restores_search` (after the same drift, search for a tag finds the item again), `health_report_counts_missing`, `health_report_on_corrupt_file_reports_integrity_error` (overwrite a page in a temp copy; expect `integrity` to be `Err`, not a panic or an `Err` from `health_report` itself). The existing migration tests still pass with the extracted fn.

### Task D2: Health check UI + Save Diagnostics Report (M)

**Files:** `crates/lumina-ui-gpui/src/app/diagnostics.rs` (new: report builder, pure), `app/preferences.rs`, `views/preferences.rs` (Storage tab: Library Health section), `app/command_palette.rs`, `menus.rs` (Help), `i18n`

**Interfaces:**
- `pub fn build_report(input: &ReportInput) -> String`, where `ReportInput { version, os_version, locale, appearance, config: serde_json::Value, health: Option<HealthReport>, log_tail: String, home: PathBuf }`. The home-to-`~` replacement runs over the whole finished text (config, health paths, log tail), not only the config.
- `fn redact_config(cfg: &AppConfig) -> serde_json::Value` is an **allowlist**: layout and display fields (`sidebar_width`, `inspector_width`, `*_visible`, `grid_columns`, `card_fit_mode`, `inspector_tab`, window geometry, `sort_*`, and the new `appearance`/`accent`/`motion`/`density`), plus `last_library_path` with home mapped to `~`. Everything else, including `last_view` (`tag:{name}`), `recent_searches`, `last_search_query`, `resume_positions`, `last_selected_media_id`, `external_editor` and any field added later, is left out.
- `fn log_tail(path, max_bytes = 256 * 1024) -> String` cuts at a line boundary and replaces the query string of every `http(s)://` URL with `?…` (signed CDN links can carry tokens).
- The report is built and the log read on a background thread (`cx.background_executor().spawn`, as in `app/active_view.rs:282`), never on the UI thread. OS version comes from `ProductVersion` in `/System/Library/CoreServices/SystemVersion.plist` read with `std::fs` (no subprocess; the only timeout helper, `run_with_timeout`, is private to `lumina-core/src/decoder.rs:32`). No environment variables are included.
- Commands "Check Library Health" and "Save Diagnostics Report…" (palette and Help menu). Selectors `preferences-health-check`, `preferences-health-rebuild-index`, `preferences-save-diagnostics`, `health-report`.
- Rebuild Search Index is a catalog write: the button is disabled while it runs, success shows a toast with the row count, and failure goes to the error banner with Retry (AGENTS §2). Nothing changes optimistically.
- The save dialog suggests the name `Lumina Diagnostics YYYY-MM-DD.txt`. The text above the button says the report includes file and folder names and addresses of imported videos and is never sent anywhere by Lumina. A write error goes to the error banner and nothing is retried silently.

- [ ] Tests (unit, diagnostics.rs): `report_omits_search_history_and_resume_positions`, `report_replaces_home_dir_everywhere` (home in config, health and log tail), `report_has_no_catalog_rows` (seeded tag, caption and `last_view = tag:…` strings are absent), `report_config_is_allowlisted` (a config with every field set yields only allowlisted keys, so a new field fails this test until it is classified), `log_tail_is_bounded_and_line_aligned`, `log_tail_strips_url_query_strings`, `report_includes_version_and_health`. UI (`tests/preferences.rs`): `health_check_shows_report_and_rebuild_button`, `rebuild_failure_shows_error_banner`. `tests/ui_thread.rs`: `health_check_never_waits_on_the_catalog_lock` (click `preferences-health-check` inside `catalog_locks_while_held`; expect 0 UI-thread locks). Keyboard: palette path test `palette_runs_check_library_health`.
- [ ] Docs: `docs/accessibility.md` is not the right home, so add a short "Getting help" section to README covering where the report goes, what it contains and what the logs contain (the "clear privacy policy for logs" in ROADMAP §11).

---

## Part E: Update check and custom shortcuts

### Task E1 (recommended, S): Help > "Check for Updates…" opens the releases page

**Files:** `menus.rs`, `actions.rs`, `app/command_palette.rs`, `i18n`, a `RELEASES_URL` const (the owner supplies the URL)

**Blocked on** plan 1's repo location and visibility; without a public URL this task does not start.

- [ ] The action runs `open <RELEASES_URL>`. The existing `reveal()` is private to `app/preferences.rs:84`; make it `pub(crate)` (it already skips under `cfg!(test)`) and record the last target in a test-only cell so a test can see what would open. Lumina sends no request, and the user's browser loads the page. Tests: `check_for_updates_opens_releases_page` (palette path; asserts the recorded target equals `RELEASES_URL`) and a menu test that Help has the item. No test that only re-reads the const.
- [ ] Alternative (owner decision, M): on click only, fetch `https://api.github.com/repos/<owner>/<repo>/releases/latest` with system curl through the timeout helper, compare it with `CARGO_PKG_VERSION` and show "Lumina X is available" with a link to the page. Never automatic, and it never downloads anything: AGENTS §5.4 says Lumina never downloads an executable at run time, so installing stays the browser's and the user's job. It needs a privacy note (the request reveals IP and app version to GitHub), an explicit opt-in per §5.4, and AGENTS §5.4 doc wording approved by the owner. Not recommended before 1.0.

**Custom keyboard shortcuts:** deferred (Decision 13). Before that it needs a keymap-as-data refactor (L) that moves `handle_key_down` matches to GPUI actions and `KeyBinding` tables.

---

## Open questions for the owner

1. **Locale:** is German acceptable as the proof locale, and who reviews the translation?
2. **Default appearance for existing users:** `System` (light users suddenly see a light app) or keep `Dark` and let users opt in?
3. **Viewer in light theme:** stays dark (recommended) or follows the theme?
4. **Accent presets:** Amber, Blue, Green, Graphite? Keep four, or offer none until asked?
5. **Density (B5):** ship, or drop as YAGNI given grid zoom?
6. **Update check:** E1 browser link (needs the public releases URL), the curl variant, or defer entirely to 1.0?
7. **Sidebar arrow-key navigation (A5)** is new behavior, so does it need design sign-off, or is a palette-only path enough?
8. **Diagnostics log tail:** include by default with the warning (recommended) or as a checkbox?
9. **Motion override (B4):** ship the in-app System/Reduce/Full choice, or only make the system setting apply without a relaunch (recommended; macOS already has the switch)?

## Out of scope

- Claiming VoiceOver support before the human A6 run; large-text or Dynamic Type scaling (ROADMAP §10 target; macOS has no system-wide text size for non-native apps, so it needs its own design); RTL layout audit (ROADMAP §10 target; no RTL locale is planned); per-control Tab stops; in-process assertions on the a11y tree (the test platform never activates a11y; add when GPUI's test platform can).
- Localizing `lumina-core` error messages and the search query syntax; more than one non-English locale; CLDR plural rules beyond one/other; locale-aware digit grouping and number formats beyond the `format_bytes` decimal separator.
- Arbitrary custom colors, the system accent color, high-contrast theme.
- Automatic, background or in-app-installing updates (1.0 roadmap); custom keyboard shortcuts.
- Uploading diagnostics anywhere; crash reporters; telemetry (AGENTS §5.4).
- Sidecar validation in the health check (needs a full walk; add when a user reports sidecar corruption).
- "Stale cache" in the health check (ROADMAP §11): the cache is already evicted automatically and Preferences › Storage shows its size with Clear Cache; add when a stale-cache bug is reported.
- Catalog backup/restore UI (ROADMAP §11): the health report shows the last backup path; restore is a separate data-safety feature.

## Review notes (2026-11-05)

Independent review against the code, gpui-pre 0.3.11 and accesskit 0.24.1 sources. Changes:

1. **Active descendant could never work as planned.** GPUI honors it only under a focused a11y node, and Lumina's focused app root has an id but no role. A1 now gives the root (and the welcome and first-scan roots) a role and label. Added the debug-build panic rule (two claimers per frame, or the focused node claiming) as a Global Constraint, with a gate in A2/A4, a test and a live-probe step.
2. **GPUI already sets app appearance** (`App::set_window_appearance`, `app.rs:1831`). Removed the planned ObjC `lumina_set_app_appearance`; corrected the "no setter" claim and Decision 6.
3. **`uniform_list` cannot take a role** (not `StatefulInteractiveElement`). Moved `Role::Grid` to `media-grid-surface`, added `Role::Row` on rows, and a `Tree` parent for sidebar `TreeItem`s.
4. **`on_click` already provides the AX press** (synthetic center click). Removed the redundant `on_a11y_action(Click)` on cards and the Decision 4 wording.
5. **B1 would have changed pixels**: `selection_wash` (12%) and `transport.rs:267` (22%) differ, so two wash tokens. `dark_palette_matches_v3_sheet` now pins every value, not 6. The literal scan also covers `hsla(`/`white()`/`black()`; `transparent_black()` (2 sites) is listed.
6. **`make bench` is core-only** (`Makefile:73–74`), so it cannot show grid render cost. Replaced it with the gate test plus a VoiceOver scroll item in A6.
7. **Diagnostics privacy:** the denylist missed `last_view` (`tag:{name}`) and any future field. It is now an allowlist with a test that fails on unclassified fields. Home becomes `~` across the whole report, URL query strings are cut from the log tail, and there is no subprocess for the OS version.
8. **D1 drift test could not run:** a contentless FTS5 table rejects `DELETE FROM`. It now uses `'delete-all'`. Documented that `integrity-check` on a contentless table does not compare against content. Added a corrupt-file test and the rebuild error path (banner, no optimistic change). The off-UI-thread test now uses the existing `catalog_locks_while_held` helper.
9. **Order:** C1 now lands before A1–A4, so labels use keys from the start. That resolves the contradiction between "A1–A4 add literals" and the new cross-plan rule. Added the Global Constraint that plans 2–5 route new strings, colors and controls through i18n/theme/a11y helpers. The C2 ratchet lands early so the baseline is fixed.
10. **i18n gaps:** chrono `%b`/`%e` English dates (3 sites), two missed plural sites, `format_bytes` decimal, `month_name` at :176 (not :178), the native menu titles in the ratchet and a drift test, `.lproj` folders so AppKit panels localize, prefix matching for `de-DE`, pure `*_in(locale)` twins for tests, and a duplicate-key test.
11. **Citations fixed:** 45 tokens (not 47); there are no `.role(` calls (the one role is passed to `InputAccessibility`); `parent_node` is at `a11y.rs:364`; contrast thresholds (subtle text is 4.0, below AA); prefs selectors now follow the existing `preferences-*` names; sidebar rows reuse `sidebar-nav-*`.
12. **A6** now covers ROADMAP §10's acceptance flow (import, export, no keyboard trap), and the probe retries after AX activation. **B4** splits the required live-refresh fix from the optional override (new owner question 9). **E1** is blocked on plan 1's repo URL, its trivia test is replaced by a behavior test, and the curl variant notes §5.4's opt-in and no-download rules.
13. **Out of scope** now gives reasons for large text and RTL, and adds stale-cache and backup/restore UI from ROADMAP §11.

Could not verify:
- ~~Whether a full scan of a contentless FTS5 table returns a row count.~~ Verified with the sqlite3 CLI: `SELECT count(*)` on a `content=''` FTS5 table returns the row count (2 after two inserts, 0 after `'delete-all'`), so D1 needs no `_docsize` fallback.
- Whether empty `.lproj` folders are enough for AppKit to pick German (C3 checks this by hand).
- Whether `Role::Application` or `Role::Group` reads better on the root. Only a VoiceOver run can tell.

The index's task count (17) is unchanged; no task was split.
