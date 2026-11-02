# Lumina Bug List & Codebase Findings

This document tracks verified bugs, UX inconsistencies, and architectural gaps in Lumina, along with exact code references and root-cause analyses.

---

## 1. Video Thumbnail Does Not Fit in Card Well (Cropped by Default)
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/config.rs:426`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/config.rs#L426) (`default_card_fit_mode`)
  - [`crates/lumina-ui-gpui/src/app/render/grid.rs:379-385`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/app/render/grid.rs#L379-L385)
- **Root Cause**:
  `default_card_fit_mode()` returns `false` (meaning square/cover). In `grid.rs`, thumbnail images are rendered with:
  ```rust
  img(render_img).size_full().object_fit(
      if fit_mode {
          ObjectFit::Contain
      } else {
          ObjectFit::Cover
      },
  )
  ```
  Since the card photo well is square (`.w_full().h(px(photo_side))`), any standard 16:9 widescreen or 9:16 portrait video thumbnail rendered with `ObjectFit::Cover` is aggressively cropped (losing ~44% of horizontal width for 16:9 videos).
- **Fix**:
  Change `default_card_fit_mode()` to `true` so all thumbnails default to `ObjectFit::Contain` ("fit by default"), keeping the entire photo/video frame visible within the card well.

---

## 2. In PiP Mode, Video Does Not Fit PiP Window (Forced 16:9 Aspect Ratio)
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/app/windows.rs:65`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/app/windows.rs#L65) (`open_floating_pip_window`)
  - [`crates/lumina-ui-gpui/src/views/floating_pip.rs:144`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/floating_pip.rs#L144)
- **Root Cause**:
  `open_floating_pip_window` hardcodes the window bounds to 16:9:
  ```rust
  let bounds = Bounds::centered(None, size(px(480.0), px(270.0)), cx);
  ```
  $480.0 / 270.0 = 16:9$. When playing portrait videos (9:16 from smartphones), 4:3 videos, or ultrawide (21:9) cinema clips, the PiP window remains rigidly locked at 480×270. Because `floating_pip.rs` uses `ObjectFit::Contain`, the non-16:9 video is rendered with massive black pillarboxes or letterboxes, making it look like it does not fit the PiP window.
- **Fix**:
  Derive the initial PiP window aspect ratio dynamically from the playing item's dimensions (`item.width` and `item.height` from `app.video.item` or `player.load()`). Scale within bounding limits (e.g., max width 480px, max height 360px) while preserving the video's native aspect ratio.

---

## 3. PiP Window macOS Traffic Light Controls Overlap Video Title
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/app/windows.rs:75-79`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/app/windows.rs#L75-L79)
  - [`crates/lumina-ui-gpui/src/views/floating_pip.rs:163-188`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/floating_pip.rs#L163-L188)
- **Root Cause**:
  `open_floating_pip_window` enables the macOS titlebar with `appears_transparent: true`:
  ```rust
  titlebar: Some(TitlebarOptions {
      title: Some("Picture-in-Picture".into()),
      appears_transparent: true,
      ..Default::default()
  }),
  ```
  On macOS, transparent titlebars draw the native traffic light handles (Close, Minimize, Zoom) in the top-left client area, spanning roughly 70px from the left edge.
  However, in `floating_pip.rs`, the title bar only has `px_2()` (8px padding):
  ```rust
  .child(
      div()
          .top_0().left_0().right_0().h(px(26.0)).px_2()
          .child(
              div().flex_1().min_w_0().truncate().child(title)
          )
  )
  ```
  The video title starts at $x = 8\text{px}$, placing the filename text directly underneath the traffic light buttons.
- **Fix**:
  Apply a left padding inset on macOS of at least 72px (`.pl(px(if cfg!(target_os = "macos") { 72.0 } else { 8.0 }))`), matching the design convention used in `toolbar.rs:113` (which insets 84px for traffic lights).

---

## 4. Media Viewer Top Bar: Title Should Only Show Date (Centered) and `(position of total)`
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/views/viewer/top_bar.rs:76-132`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/viewer/top_bar.rs#L76-L132)
- **Root Cause**:
  Currently, `viewer/top_bar.rs` renders a cluttered, left-aligned `center_section` containing:
  1. Date prefix (`month_name(item.month) item.year`)
  2. Chevron icon (`Icon::ChevronRight`)
  3. Bold filename (`item.file_name`)
  4. Technical specs line (`specs_line(item)`)
  5. Position indicator (`"3 of 5"`)
  The layout is left-aligned (`flex-row items-center gap-1.5 flex-1 min-w-0`), rather than centered over the viewing area.
- **Fix**:
  Simplify the center section to only render the formatted date (e.g. `November 2, 2026`) and `(X of Y)` position (e.g. `(3 of 5)`). Center the container with `.justify_center()` so the title is neatly positioned in the middle of the window header.

---

## 5. Explore Removing Duplicated Filter Bar & Fit/Square Toggle (Full Bar Removal)
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/app/render/mod.rs:494`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/app/render/mod.rs#L494) (`SearchBarView::render(self, cx)`)
  - [`crates/lumina-ui-gpui/src/views/search_bar.rs:184-236`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/search_bar.rs#L184-L236)
- **Root Cause / Opportunity**:
  `SearchBarView::render` inserts an entire 44px filter bar (`#filter-bar`) between the top toolbar and the media grid. This bar contains:
  1. Filter chips (`All`, `Favorites`, `Videos`, `3+ Stars`, `Labeled`) — which duplicate the sidebar albums and search query filters.
  2. `[ Fit | Square ]` toggle — redundant if thumbnails fit by default.
  3. Thumbnail size slider — redundant with keyboard shortcuts (`Cmd +` / `Cmd -`), trackpad pinch-to-zoom (`Cmd + Scroll`), and Command Palette.
  Removing this bar reclaims 44px of vertical screen real estate, eliminates redundant controls, and provides an uncluttered photo-first interface.
- **Fix**:
  Remove `SearchBarView::render` from `app/render/mod.rs`. Ensure `default_card_fit_mode()` is `true` (fit by default) and `default_grid_columns()` is set to a sensible default (e.g. 4 columns). Update tests that assert `#filter-bar` bounds.

---

## 6. Remove Redundant "Manage…" from Sidebar Tags Header
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/views/sidebar/sections.rs:365-377`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/sidebar/sections.rs#L365-L377)
  - [`crates/lumina-ui-gpui/src/app/render/toolbar.rs:234-241`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/app/render/toolbar.rs#L234-L241)
- **Root Cause**:
  `sections.rs` adds a `"Manage…"` link directly beside the `TAGS` header in the sidebar. Meanwhile, the primary toolbar already provides a prominent Tags button (`tags-toolbar-btn` with `Icon::Tag` and tooltip `"Tags"`), and the Command Palette offers "Manage Tags". Having a second textual "Manage…" button in the narrow sidebar is redundant and visually inconsistent with other sidebar section headers.
- **Fix**:
  Remove the `open-tag-manager-sidebar-btn` child from the `TAGS` section header in `sections.rs`, leaving just the clean section label `theme::section_label("TAGS")`.

---

## 7. Video Scrub Bar Hover Causes Layout Jump
- **Status**: Verified in code
- **Files**:
  - [`crates/lumina-ui-gpui/src/views/viewer/transport.rs:166-188`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/viewer/transport.rs#L166-L188)
- **Root Cause**:
  The `video-transport` container is configured as a flex column with gap:
  ```rust
  div()
      .id("video-transport")
      .relative()
      .flex()
      .flex_col()
      .gap_2()
      .child(if let Some((badge_pct, badge_time)) = time_badge {
          div()
              .id("video-time-badge")
              .absolute()
              .top(px(-26.0))
              ...
      } else {
          Empty.into_any_element()
      })
      .child( /* Scrubber track row */ )
  ```
  When the mouse is NOT hovering over the scrubber track:
  - `time_badge` is `None`.
  - Child 0 is `Empty.into_any_element()`.
  - In GPUI / Taffy flexbox layout, `Empty` is an in-flow element of size 0. Because the container has `.gap_2()` (8px), Taffy places an 8px flex gap between Child 0 (`Empty`) and Child 1 (the scrubber track row).
  When the mouse moves over the scrubber track:
  - `on_mouse_move` triggers `set_video_hover_scrub`, setting `time_badge` to `Some(...)`.
  - Child 0 becomes `video-time-badge`, which has `.absolute()`.
  - Being positioned absolutely, Child 0 is removed from flow layout.
  - Child 1 (scrubber track row) now becomes the *first* in-flow element.
  - The preceding 8px flex gap disappears immediately, causing the scrubber track and transport controls to visibly jump upward by 8px!
  - When the mouse leaves the scrubber, Child 0 reverts to `Empty`, and the entire scrubber row jumps back down by 8px.
- **Fix**:
  Do not inject `Empty` as an in-flow flex sibling. Instead, place `video-time-badge` inside the `.relative()` scrubber track container itself, or mount it conditionally via `.when_some(time_badge, ...)` outside in-flow layout calculations.

---

## 8. Default Grid Columns (5) Mismatches `SIZE_COLUMNS` Levels
- **Status**: Verified in code (New finding)
- **Files**:
  - [`crates/lumina-ui-gpui/src/config.rs:422`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/config.rs#L422) (`default_grid_columns`)
  - [`crates/lumina-ui-gpui/src/views/search_bar.rs:293-303`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/search_bar.rs#L293-L303) (`SIZE_COLUMNS` and `size_level`)
- **Root Cause**:
  `default_grid_columns()` initializes new library configurations to `5` columns:
  ```rust
  fn default_grid_columns() -> usize { 5 }
  ```
  However, the discrete grid zoom levels are defined as:
  ```rust
  pub(crate) const SIZE_COLUMNS: [usize; 5] = [8, 6, 4, 3, 2];
  ```
  Because `5` is not in `SIZE_COLUMNS`, `size_level(5)` fails to find an index and falls back to `unwrap_or(SIZE_COLUMNS.len() - 1)`, which evaluates to level 4 (2 columns!).
  When a user opens a fresh installation and hits `Cmd +` or touches any zoom control, the grid abruptly leaps from 5 columns directly to 2 columns.
- **Fix**:
  Change `default_grid_columns()` to `4` (the middle level in `SIZE_COLUMNS`), ensuring seamless stepping in both zoom directions.

---

## 9. PiP Window Titlebar Drag Interception
- **Status**: Verified in code (New finding)
- **Files**:
  - [`crates/lumina-ui-gpui/src/views/floating_pip.rs:168`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/floating_pip.rs#L168)
- **Root Cause**:
  In `floating_pip.rs`, the titlebar handles mouse down with:
  ```rust
  .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
  ```
  The comment states this is to prevent mouse clicks on controls from passing through to the underlying video surface (which starts a window move). However, because `stop_propagation()` is placed on the entire title bar container, clicking and dragging on the title bar itself is blocked. Users naturally attempt to reposition floating windows by dragging their titlebar; currently, only clicking on the video content moves the window.
- **Fix**:
  Allow `window.start_window_move()` when clicking on empty titlebar regions, while stopping propagation only on interactive button children.

---

## 10. Redundant Close Controls in PiP Window
- **Status**: Verified in code (New finding)
- **Files**:
  - [`crates/lumina-ui-gpui/src/views/floating_pip.rs:208-217`](file:///Users/raymondottun/Projects/lumina/crates/lumina-ui-gpui/src/views/floating_pip.rs#L208-L217)
- **Root Cause**:
  Because macOS transparent titlebars are enabled, macOS renders the red Close traffic light in the top-left corner. At the same time, `FloatingPipPlayerView` renders a custom Close button (`pip-close-btn` with `Icon::X`) in the top-right corner. Having two close buttons in a small PiP overlay creates visual clutter.
- **Fix**:
  Consolidate window dismissal: retain the native macOS traffic lights (with proper inset) and remove the redundant custom `pip-close-btn`, or use custom window controls uniformly.
