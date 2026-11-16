# Design 18: Settings — General — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle Preferences › General to the design's grouped-row settings page, with category navigation, toggles, segmented controls and selects, building only the settings Lumina can honestly offer.
**Design:** `lumina.pen` frame `l85pQN` "Settings — General" (export with the pencil MCP to view).
**Depends on:** Design 00 Foundation (tokens, fonts, `theme::nav_item` for categories, Sidebar shell) and Design 01 Task 1 (`page_header`, the TopBar of every page). Design 15 fills the Library, Downloads and Storage categories of this shell and uses the Task 2 widgets.
**Icons (fetched in 00 Task 1):** refresh-cw (Reset to defaults). Already in `resources/icons`: settings, info, folder, x. Wrench (Rebuild index) moves to Design 15 with the Library rows.
**App today:**
- Preferences is a 680 px modal over the library: `render_preferences_modal` in `crates/lumina-ui-gpui/src/views/preferences.rs:11-176`.
  - Three icon tabs, General, Storage and About, each 84×54 (`:16-79`).
  - A 38 px title bar with a close button (`:121-161`).
  - General (`:180-231`) has Library folder with Change…, Items indexed with Rescan Library, and External editor with Choose Editor….
  - Storage (`:233-301`) and About (`:303-367`).
  - Row helpers: `section_title`, `group`, `row` (44 px, label 130 wide), `value`, `note` and `button` (`:376-458`).
  - It is mounted from both render roots: the library layout (`src/app/render/mod.rs:451-452`) and the no-library (welcome) layout (`:864-865`), covered by `src/tests/welcome.rs:711` (`welcome_preferences_paint_and_escape_closes`).
- State and actions: `src/app/preferences.rs:10-16` (`PreferencesTab`) and `:94-260`.
- Keys: ⌘, and ⌘K Preferences (`src/app/keymap/mod.rs:226-228`, `src/app/command_palette.rs:305`). Inside the sheet only Esc works (`src/app/keymap/modals.rs:72-78`), so switching tabs is mouse-only today.
- Tests: `src/tests/preferences.rs`.

**Export note:** `l85pQN.html` is byte-identical to `NWldm.html` (191,493 bytes), but the frame is not a copy of "States — Error". Each HTML export contains both frames. "Settings — General" is the second one, from line 1037 (`data-pencil-name="Settings — General"`, positioned at left 1600, top 1100). The two PNGs differ and each matches its own frame, so this frame is finished. The values below come from lines 1037–2219 of that file.

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data (e.g. "48,291 items", "Lisbon"): render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views.
- **AGENTS.md §1 and §5.4 forbid several rows in this frame:**
  - no accounts;
  - no telemetry, analytics or crash uploaders;
  - no background network (update checks only as explicit opt-in, uploading no user data);
  - no two-way sync. S3 backup is planned for 0.5 and is not built.

  Do not build the Account card, "Share anonymous diagnostics", "Send crash reports" or "Install updates automatically" without explicit product approval and an AGENTS.md change.
- Tests must not persist config (`save_config` returns early under `#[cfg(test)]`).

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Container | Modal card 680 wide, height clamped 360–600, dim backdrop (`preferences.rs:87-118`) | Full window. Its own 252 px sidebar (`bg_1`, right `line` border, padding 16×12, gap 18) next to a main column | relayout (decision) |
| Settings sidebar: brand and search | none | Wordmark "Lumina" (Inter Tight 18/600). Search field h 34, r 8, `bg_0`, `line` outline, placeholder 12.5 t3 "Search settings" | wordmark new; search field dropped (lead decision) |
| Settings sidebar: categories | Icon tabs 84×54: General, Storage, About (`preferences.rs:26-79`) | "SETTINGS" label (10/600, +0.9 tracking, t3). NavItems h 32, r 8, px 10, gap 10, label 13: selected `bg_3`, t1, weight 500; others t2. Items: General, Library, Import, Downloads, Sync & Backup, Appearance, Shortcuts, Privacy, Advanced | relayout to `theme::nav_item`; pinned order General, Library, Downloads, Storage, About (Advanced dropped: empty) |
| Settings sidebar: library meter | none | "Library size" (11 t2), mono 10 t3 "412 GB / 1 TB", 4 px track `bg_3` with accent fill | not in app (decision) |
| Title bar | 38 px, centred tab name, 20 px close × (`preferences.rs:121-161`) | `page_header` (01 Task 1): TopBar 67.5 h, px 28, bottom `line`. Heading "General" (Inter Tight 20/600, −0.3). Meta "Account, library location, language and updates" (12 t2). Right: "Reset to defaults" (h 32, r 8, px 12, `bg_2`, `line` outline, refresh icon, 12.5) | relayout + copy |
| Content | `px 26`, `py 22`, scroll (`preferences.rs:163-174`) | padding 16×32, gap 14, scroll | restyle |
| Account card | none (About says "No account, no network, no telemetry", `preferences.rs:343-346`) | Avatar 48 (accent-soft, accent outline, initials). Name (Inter Tight 15/600). "Lumina Pro" badge. Email, member-since and devices (mono 11). Storage "412.8 GB of 1 TB" with a 180×5 bar. "Manage plan" | remove from design (AGENTS.md) |
| Section label | `theme::section_label`, mt 22, mb 8 (`preferences.rs:376-380`) | Inter 10/600, +0.9 tracking, t3, gap 9 above the list. An optional right note (11.5 t3) | restyle |
| Group | r LG, `border_subtle`, `bg_app`, dividers (`preferences.rs:383-399`) | `bg_1`, r 12, 1 px `line` outline, padding 0 16. Rows separated by a bottom `line` border | restyle |
| Row | min-h 44, px 14, label 130 wide in `text_muted`, value in the middle, buttons (`preferences.rs:401-418`) | Label column flex 1 (13/500 t1), optional description (11.5/400 t2, line-height 16). Control on the right, gap 24. Height 43.5 (toggle), 55.5 (control) or 62.5 (with description), padding 11 or 14 vertical | relayout |
| Row button | `theme::control` (`preferences.rs:445-458`) | h 34, r 9, px 13, `bg_3`, `line_2` outline, 12.5/500 t1, optional icon | restyle |
| Path value | mono `text_xs` truncating (`preferences.rs:420-432`) | Pill h 34, r 9, `bg_0`, `line_2` outline, folder icon, mono 11.5 t2 | restyle |
| Library location | "Library folder" plus "Change…" opens another library (`preferences.rs:187-193`) | "Library location" with the description "Moving the library copies 412.8 GB — keep the Mac plugged in.", the path pill and "Choose…" | moves to Design 15's Library category (lead decision). Note for 15: the description must be honest; Change opens another library and moves nothing |
| Library size | "Items indexed: N · last scan 3m ago" plus "Rescan Library" (`preferences.rs:194-205`) | "Library size": mono 11.5 t2 "412.8 GB · 48,291 items", and "Rebuild index" with a wrench icon | moves to Design 15's Library category |
| Language | none (only en-US ships, AGENTS.md §11) | Select 240×34, r 9, `bg_0`, `line_2` outline, globe icon, "English (United Kingdom)", chevron | not in app (decision) |
| Date & time format | none | Select "13 Jun 2026 · 24-hour" with a calendar icon | not in app (decision) |
| Open Lumina at login | none | Toggle 40×22, r full, padding 2. On: accent fill with an 18 px knob in accent-fg. Off: `bg_3` fill, `line_2` outline, knob t3 | not in app (decision) |
| Default view on launch | always restores the last view (`config.rs`, `last_view` and `RestoredSession`) | Description "Lumina reopens here after a restart."; segmented control `bg_0`, r 9, `line_2` outline, padding 3, gap 2; segments h 28, r 6, px 12 (selected `bg_3` t1/500, others t3): Library / Last viewed / Albums | new setting (S) |
| Include items on connected drives | none | toggle | not in app (decision) |
| UPDATES section | version only in About (`preferences.rs:323-329`) | Note "Lumina 3.4.1 (build 2026.11)"; Update channel Stable/Beta; Install updates automatically; "Last checked 14 minutes ago" plus "Check now" | not in app (decision); show the real version in the section note only |
| PRIVACY section | none | "Share anonymous diagnostics" (description "Usage counts only…") and "Send crash reports" toggles | remove from design (AGENTS.md) |

## Not in the app yet (needs a product decision)
- **Full-window settings vs the modal sheet.** The design replaces the library layout with a settings sidebar and main column. Size M: a new top-level mode like the welcome screen, or a separate GPUI window (`src/app/windows.rs` already opens floating windows). It also changes the test contract (`preferences-card`, the 560 px fit test in `tests/preferences.rs:7-68`). **Decided by the lead:** an in-window, full-window mode built in Task 1. With no library open it fills the window with its own category sidebar. Esc closes and ⌘, toggles; there is no ⌘W (no binding exists).
- **Categories (decided).** Pinned order: General, Library, Downloads, Storage, About. General (this plan) holds general rows only. Library (the library rows and Library size), Downloads and Storage (today's Storage rows) belong to Design 15. Advanced appears only if rows remain after those moves; with the External editor in General, it is empty and is dropped. Import, Sync & Backup, Appearance, Shortcuts and Privacy are not shown (no settings exist; Sync & Backup is the 0.5 S3 plan).
- **Search settings (decided):** no settings search field. The design also repeats "Search settings" as the shortcut text, which looks like a design slip.
- **Sidebar "Library size 412 GB / 1 TB" meter.** Owned by Design 15 (library bytes and volume capacity through the existing native bridge, no new crate).
- **Language and Date & time format.** Only en-US ships. Recommendation: drop Language until a second locale exists. Date format is M: a config field plus every formatter, `formatted_timestamp` among them. Separate plan if wanted.
- **Open Lumina at login.** `SMAppService` needs macOS 13, and the app supports 12 (`resources/Info.plist:25-26`). Size M, platform code. Recommendation: separate plan, or drop it.
- **Default view on launch.** Size S: a config enum (`Library | LastViewed | Albums`, default `LastViewed` = today's behaviour) read in startup restore. Recommendation: build it here. "Albums" needs a defined target (the first album? the album list?), so ask the designer or ship Library and Last viewed only.
- **Include items on connected drives.** Unclear semantics (the scanner only walks the library root). Recommendation: drop it.
- **Updates (channel, auto install, check now).** No updater exists. Auto-install implies background network, which AGENTS.md forbids. Recommendation: separate plan with product approval, offering only "Check now" (explicit, opt-in). Show the version note now.
- **Reset to defaults.** `settings-reset` is asserted only in Task 4. Which settings? Layout widths, sort and grid density are safe. The library path, recents and resume points are not. Size S, with a confirmation dialog. Recommendation: build it here, scoped to UI preferences, with the confirmation.

## App has, design lacks
- **External editor** (Choose Editor…, `preferences.rs:206-225`). Keep it in General under an "EXTERNAL EDITOR" section. `preferences-choose-editor` is tested.
- **Storage rows** (thumbnail cache size, Reveal Cache, Clear Cache with the "Cleared" confirmation, Reveal Config, Reveal Logs; `preferences.rs:233-301`). Kept, with all their selectors, in Design 15's Storage category.
- **About** (version, licence, acknowledgements; `preferences.rs:303-367`). Keep it as the last category. The bundled-notices requirement is in AGENTS.md §15.
- **The close button** (`preferences-close`, tooltip "Close (Esc)"). Keep a visible close or back control: in a full-window mode it is the only pointer way out.

## Review Focus
- Keyboard: ⌘, toggles. ↑/↓ move between categories, Tab walks the controls, Space flips a focused toggle, ←/→ move the segmented control, and Esc closes. Library shortcuts stay absorbed while settings is open (`modals.rs:72-78`).
- A long library path (deep `/Volumes/...`): the pill truncates from the start or middle and keeps a tooltip with the full path. "Choose…" never wraps.
- A small window (1024×560, as the existing test uses): the content scrolls and the TopBar and the sidebar categories stay visible.
- No library open: settings open from the welcome screen as a full window with their own category sidebar (`render/mod.rs:864-865`); General works without a library and Esc returns to the welcome screen.
- A failed config write (Default view, Reset): show the error banner and revert the control (AGENTS.md §2).

## Tasks
### Task 1: Settings shell (sidebar categories, TopBar, content)
**Files:** Modify `src/views/preferences.rs:11-176` (split it into `views/preferences/{mod,general,about}.rs`), `src/app/preferences.rs:10-16` (categories), `src/app/keymap/modals.rs:72-78`, `src/app/render/mod.rs:451-452` and `:864-865` (both roots mount the full-window mode instead of the overlay) / Test `src/tests/preferences.rs`, `src/tests/welcome.rs:711`
- [ ] Write a failing test `settings_shell_has_sidebar_categories_and_keyboard_nav`. After ⌘,, `settings-sidebar` is 252 ±1 wide, with `settings-category-general` and `-about` in the pinned order (Design 15 adds `-library`, `-downloads` and `-storage` between them). `down` moves to the next category. `escape` closes and `secondary-,` toggles. Update `welcome_preferences_paint_and_escape_closes` so that, with no library, `settings-sidebar` fills the window and Esc returns to the welcome screen. Update `preferences_tabs_group_actions_and_fit_a_small_window` to target the new selectors (a test-contract change; say so in the commit).
- [ ] Implement: the layout values from the table. Use `theme::nav_item` for categories, `page_header(title, meta, actions)` (01 Task 1) for the TopBar per category, and content padding 16×32. Keep `preferences-close`.
- [ ] Run `cargo test -p lumina-ui-gpui preferences`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit `feat(design): settings shell`, staging explicit paths.

### Task 2: Row, group, toggle, segmented and select widgets
**Files:** Modify `src/views/preferences/mod.rs` helpers (replacing `preferences.rs:376-458`), `src/theme.rs:320-350` (`segmented` and `segmented_item`; also used by `views/tag_manager/list.rs` and `views/viewer/stage_controls.rs`, so check both after the restyle) / Test `src/tests/preferences.rs`
- [ ] Write a failing test `settings_rows_match_design_heights`. A plain row is 43.5 ±1 tall, a row with a description is 62.5 ±1, and a row button is 34 tall.
- [ ] Implement: `group`, `row(label, desc)`, `row_button`, `path_pill`, `toggle(id, on)` (40×22, Space and click, `Role::Switch` with an aria label) and a select row. Restyle the existing `theme::segmented`/`segmented_item` to the design (container `bg_0`, r 9, `line_2` outline, padding 3, gap 2; segments h 28, r 6, px 12) instead of adding a new one, and give it ←/→ and `Role::RadioGroup` in the settings rows. This task owns these widgets; Design 15 uses them.
- [ ] Run `cargo test -p lumina-ui-gpui settings_rows`; expect pass.
- [ ] Gate and commit `feat(design): settings row widgets`.

### Task 3: General page content
**Files:** Modify `src/views/preferences/general.rs`, `src/config.rs` (`launch_view`), the startup restore in `src/app/library.rs` / Test `src/tests/preferences.rs`, `src/tests/session_restore.rs`
- [ ] Write a failing test `general_shows_general_rows_and_launch_view`. It shows `settings-launch-view`, `preferences-choose-editor` and `settings-version-note` (the real version); the library rows (`preferences-change-library`, `preferences-rescan`) are not on General (they are on 15's Library page). Choosing "Library" in `settings-launch-view` sets `config.launch_view`. A restored session with `Library` opens All Media instead of the last view.
- [ ] Implement: the GENERAL section (Default view on launch), the EXTERNAL EDITOR section, and the UPDATES note with `env!("CARGO_PKG_VERSION")`. Library rows and Library size are Design 15's. No Account, Privacy toggles, Language, Date format, login item or update controls (see decisions).
- [ ] Run `cargo test -p lumina-ui-gpui general_shows` and `cargo test -p lumina-ui-gpui session_restore`; expect pass.
- [ ] Gate and commit `feat(design): settings General page`.

### Task 4: Reset to defaults (if approved)
**Files:** Modify `src/app/preferences.rs`, `src/config.rs` / Test `src/tests/preferences.rs`
- [ ] Write a failing test `reset_to_defaults_asks_then_resets_ui_preferences_only`. `settings-reset` exists in the TopBar actions; after confirming, the sidebar width and grid columns are back to their defaults, and `last_library_path` and `recent_libraries` are unchanged. Cancel changes nothing.
- [ ] Implement it, with a confirmation that uses the existing confirm-dialog pattern (`views/smart_album_delete.rs::confirm_dialog`).
- [ ] Run `cargo test -p lumina-ui-gpui reset_to_defaults`; expect pass.
- [ ] Gate and commit `feat(design): reset settings to defaults`.

Manual visual checks: the toggle and segmented colours, the group radius and outline, the TopBar spacing, and the sidebar selected state. Compare them against `l85pQN.png`. In the PNG, "Downloads" also has the selected fill next to "General". That looks like a hover or design slip, so ask the designer, and do not show two selected categories.
