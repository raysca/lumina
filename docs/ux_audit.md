# UX Audit: Everyday Interactions

Status: current audit of the v0.2.0 macOS app. This is the detailed companion to
the UX workstream in [ROADMAP.md](../ROADMAP.md).

## Why this audit exists

Lumina already has the core media-manager features. The remaining gap is the
everyday interaction layer: how selection feels, how a photo opens, how motion
explains what changed, how zoom tracks the pointer, how video scrubbing feels,
and how the app restores context. These are the details users notice on day one
even when the feature list is strong.

## Interaction rules

1. **Single click selects; double click opens.** Selection is immediate and
   never opens the viewer. A double click opens the viewer. Enter and Space also
   open. Escape closes.
2. **Every state change has feedback.** Hover, press, selection, focus, loading,
   empty, success and failure states are explicit and consistent.
3. **Motion explains continuity.** An item opens from its card, closes back to
   it, and reduced-motion users get the same result without animation.
4. **Navigation does not lose context.** Back, restored view, selection and
   scroll position are expected across viewer, albums and relaunch.
5. **Zoom anchors on intent.** Grid zoom follows the pointer or keyboard focus;
   viewer zoom follows the cursor and can be reset predictably.
6. **Scrubbing is direct manipulation.** The video timeline responds while
   dragging, shows the destination frame, and never stalls the UI.
7. **No dead ends.** Empty states offer the next action. Long jobs have progress,
   cancel and retry. Errors state what failed and what to do.
8. **Keyboard and mouse share the same state.** A selection or viewer opened one
   way must behave identically the other way.

## 1. Navigation and context

**Current**

- Grid navigation: arrows, Home/End, PageUp/PageDown, Cmd+A, Escape.
- The media viewer is a fullscreen overlay over the grid. Arrows and `[`/`]`
  navigate items, the viewer Back button (or Escape) returns to the grid, and
  stage prev/next buttons step through items (v3 removed the filmstrip).
- Breadcrumb is display-only. Back and Forward history exist (Cmd+[ / Cmd+],
  Go > Back / Forward, the Cmd+K "Back" and "Forward" rows, and the fullscreen
  viewer's Back button). A new destination after Back drops the Forward
  entries, as in a browser. There is no recent view or clickable breadcrumb.
- Relaunch restores the last view, search, year, selection and grid scroll
  position (anchored on the first visible item, so it survives column and
  density changes). The restore waits for the startup scan and is dropped if
  the user navigated, typed a search or selected a card first. Quit saves the
  session. The viewer is not reopened; Enter opens the restored selection.
- Closing the viewer leaves the grid where it was (the grid stays mounted).
- Sidebar sections do not collapse; albums/tags cannot be pinned or reordered.
- Switching album clears search, year and selection; Back returns to the
  previous view. Opening another library starts a fresh Back/Forward trail.

**Missing / weak**

- No branching trail; Back/Forward is a single linear stack (50 entries).
- No recent views (recent libraries are on the welcome screen and in Cmd+K;
  see §9).
- No "reset restored view" action (the sidebar, Cmd+K Go to rows and Cmd+[
  return to All Photos).
- No clickable breadcrumb or "return to album" affordance.
- No jump-to-date.
- No command-palette navigation history or recent commands.

**Target**

- Cmd+[ and the viewer Back button move back across album, search, viewer and
  settings.
- Persist last library, album, search, selection and scroll position; restore on
  launch with a clear reset action.
- Breadcrumb segments clickable; the viewer Back button returns to the source
  view.
- Date jump.
- Recent libraries and recent views with remove/clear.

**Acceptance:** opening a photo and pressing Escape returns to exactly the same
grid position and selection; relaunch restores the previous browsing context;
Back never traps the user.

## 2. Selection and open semantics

**Current**

- A single click selects the item immediately. Shift-click extends a range;
  Cmd-click toggles an item.
- A double click without Shift/Cmd opens the viewer. The first click of the
  double click selects, then the second opens.
- Keyboard arrows move selection; Enter and Space open the viewer.
- Clicking empty grid space does not clear selection; there is no background
  click handler.
- No rubber-band drag selection, Shift+arrow range extension, invert selection,
  or select-similar.
- Card selection is indicated by a border; hover only changes border color. There
  are no hover quick actions.

**Target**

- Keep the single-click selects / double-click opens contract. Ensure a slow
  double click cannot accidentally open when the user intended selection.
- Clicking empty space clears selection; Escape progressively clears selection,
  search, inspector and context menu in a predictable order.
- Shift+arrow extends selection; Shift-click and Cmd-click remain consistent.
- Rubber-band selection for pointer users; select-all-in-view and invert.
- Hover quick actions: favorite, select, open, more menu; tooltip for truncated
  filenames. Keep them keyboard-equivalent.
- Selected, focused, hovered and pressed states are distinct at a glance.

**Acceptance:** a user can select, extend, toggle, clear and open entirely by
mouse or keyboard; no single click opens; no double click merely selects;
selection survives resize and focus changes.

## 3. Opening, closing and motion

**Current**

- The viewer fades in on open (160 ms, `motion::overlay_in`, also under
  Reduce Motion) and closes instantly. There is no shared-element zoom from
  the card and no crossfade between viewer items (GPUI has no layout or exit
  animation).
- While a frame decodes, the viewer keeps the full stage: the cached grid
  thumbnail, else a neutral full-stage canvas (`viewer-stage-placeholder`),
  so opening and stepping never show a blank box or jump in size. The
  thumbnail-to-full-frame swap has no fade. Opening or stepping prefetches
  the grid thumbnails of the items before and after.
- An item whose decode failed shows a full-stage "Lumina can't show a
  preview of this file." with "Press O to open it in its default app."
  (`viewer-stage-failed`).
- Menus, dialogs, the palette and toasts fade in (`motion.rs`); toasts also
  rise unless Reduce Motion is on. Decoded thumbnails fade in over a static
  skeleton card.

**Target motion system**

- Viewer opens from the selected card bounds; closes back to the current card.
- Crossfade/slide between viewer items with adjacent prefetch; cancel cleanly on
  rapid navigation.
- 160-240 ms ease-out for overlays and panels; 120-180 ms for selection and
  hover; no animation longer than 300 ms.
- Sidebar collapse, inspector slide, modal fade/scale, command palette and toasts
  use the same timing and easing tokens.
- Thumbnail fade-in and subtle skeleton shimmer; never block input.
- Respect reduced motion: all transitions become instant or a short opacity
  change, and media autoplay behavior is unchanged.

**Acceptance:** no blank frame during open/close; rapid navigation does not
leave a stale viewer; reduced motion disables movement; no animation causes
layout shift or blocks input.

## 4. Zoom and pan

**Current**

- Grid zoom is 2-8 columns via ⌘+/⌘-, the toolbar slider and ⌘-scroll
  (anchored at the pointer); no pinch, no animated reflow.
- Viewer zoom: pinch and ⌘-scroll zoom anchored at the pointer; a plain
  scroll or drag pans while zoomed. Presets: `⌘0` Fit, `⌘1`/`⌘2`/`⌘4`
  100%/200%/400% of actual pixels and `⌘3` Fill, also in ⌘K and (while
  zoomed) as Fit · Fill · 1:1 buttons under the navigator. `Z` and
  double-click toggle Fit/100%; pan stops at the media edges. Plain digits do
  not zoom (they are reserved for ratings). Stills decode at most 2048px, so
  100% of larger photos is an enlarged decode.
- While zoomed the HUD shows a navigator with the percentage and a Fit
  button. No animated reset, no pan inertia, no smart zoom.

**Target**

- Grid zoom follows pointer/keyboard focus and keeps the same item under the
  pointer; animate reflow with reduced-motion fallback.
- Viewer zoom anchors on the cursor; double-click toggles fit/100%; presets for
  Fit, 100%, 200%; pinch gesture where available.
- Zoom HUD with percentage and reset; pan constraints keep the image reachable.
- Smooth zoom/pan with no frame drops; no overshoot at fit boundaries.

**Acceptance:** zooming never jumps to a different photo; the pointer target
stays stable; reset always returns to fit; keyboard-only users can reach every
zoom preset.

## 5. Video scrubbing and playback

**Current**

- The scrubber is a segmented click target. Hover sets a time/percentage; the
  hover preview is a generated gradient placeholder, not a decoded video frame.
- Clicking seeks; there is no drag-scrub, no release-to-commit behavior, no
  buffered range, and no frame preview while dragging.
- HUD has explicit controls but no timed auto-hide; resume position is not
  persisted; no chapters, bookmarks, audio tracks, subtitles, loop range, or
  trim.
- Playback speed/volume are stateful for the session but not persisted; frame
  stepping exists via `,`/`.`; J/K/L and arrows seek.

**Target**

- Pointer-down starts direct scrubbing; pointer-move seeks with a real thumbnail
  preview and timestamp; release commits. Playback pauses or slows during scrub
  to avoid audio thrash.
- Draggable playhead, buffered range, chapter markers, hover preview.
- Resume last position per video; persist speed/volume defaults.
- Auto-hide HUD after inactivity; move mouse or press a key to reveal.
- Keyboard: frame step, 5 s seek, play/pause, mute, loop, fullscreen, PiP, with
  visible feedback.
- Later: audio/subtitle track selection, loop range, trim/export.

**Acceptance:** scrubbing is smooth and shows the correct destination frame; no
UI freeze; resuming a video returns to the saved position; keyboard and pointer
produce the same seek behavior.

## 6. Feedback, loading and empty states

**Current**

- Thumbnails show a static skeleton card until loaded (no shimmer) and fade
  in. A thumbnail that fails to decode (unreadable, offline or unsupported
  file) shows a failed card instead: an alert icon and the caption "No
  preview", with a tooltip saying Rescan tries again. It is not decoded again
  until the file changes or the library is rescanned (⌘R). There is no
  per-card retry; Reveal in Finder stays available.
- Empty states have an icon, a title, a sentence and, where one helps, an
  action: Show all photos, Edit Rules… for a smart album, Manage tags, Rescan
  or Choose Another Folder… for an empty library, and a removable chip per
  search term. Recently Deleted and Duplicates explain themselves without a
  button.
- Status bar shows the last action indefinitely; errors use a dismissible banner.
- One toast at a time (no stack), with Undo for reversible actions; it closes
  after 8 s unless hovered. No job center. Export and XMP import run in the
  background with a status-bar line ("Exporting 40 items…", "Reading XMP for
  12 items…") and a final toast or error banner; a second one while the first
  runs is refused with a toast. There is no per-item progress or cancel, and
  Quit does not wait (at most one partial copy at the export destination).
  The first scan of an empty library shows a progress screen (count, current
  file, time estimate) with Stop Scan; rescans of a populated library show
  only a status-bar count, with no cancel. Watcher changes are silent.

**Target**

- Skeleton cards with stable dimensions, fade-in thumbnails, and an error icon
  with retry/reveal.
- Actionable empty states: open folder, import, clear search, show all, edit
  smart-album rules, empty trash.
- Toast stack for success/failure with Undo where reversible; errors include an
  action.
- Background job center for import, export, scan and backup; progress, cancel,
  retry, per-item failures.
- Watcher summary: "12 new items added" with dismiss.

**Acceptance:** every operation longer than 300 ms has visible progress; every
failure is actionable; no empty state is a dead end; long jobs never block the
UI thread.

## 7. Search, filters, tags and albums

**Current**

- No quick-filter chips: filtering runs through the search field and its
  operator suggestions (is:favorite, is:video, is:raw, has:location, has:tag,
  has:color, rating:>=4).
- Search suggestions and recent searches exist; saved searches, query
  explanation and date operators do not.
- Tags list in the sidebar, but no Untagged node, no right-click rename/delete,
  and rename is only in the tag-manager modal.
- Smart albums: no cover preview, pinning, reordering, descriptions, duplicate,
  rule import/export; deletion is immediate without confirmation/undo.
- No date section headers or timeline scrubber in the grid.

**Target**

- Keep the filter vocabulary discoverable through search suggestions; add date
  operators and query explanation.
- Search suggestions for tags, cameras, lenses, cities and operators; recent and
  saved searches.
- First-class Untagged; tag right-click menu; drag tags onto selection; hierarchy
  and aliases; undo for rename/merge/delete.
- Smart-album preview, pin, duplicate, description, rule import/export, and
  confirm/undo on delete.
- Grouped timeline headers in the grid and a visible scrubber.

**Acceptance:** filters never contradict the query text; a user can build a
filter without memorizing syntax; tag and album edits are reversible.

## 8. Batch editing and context menus

**Current**

- Batch dock covers favorite, tags, trash, export and clear; inspector covers
  batch rating and color. No batch title, date, location, caption, keywords or
  metadata templates.
- Mixed values are not shown; no apply-to-N confirmation.
- Batch undo is per item, not one grouped action.
- Context menu lacks Share, Copy Image, Edit, Open in External Editor, Move,
  Rename and curated-album add.

**Target**

- Mixed-value inspector with explicit "apply to N" and one grouped undo entry.
- Batch metadata/keywords, caption, date and location with templates.
- Context menu covers primary actions; keyboard reachable.
- Drag selection to export, trash, album or tag.

**Acceptance:** no accidental partial edits; undo restores all touched items in
one step; every context action is discoverable from the command palette.

## 9. Onboarding, import and first-run

**Current**

- One welcome screen with three paths, Open Existing Library… (↵), Import
  into New Library… (⇧⌘I, also File > Import… there) and Explore a Sample
  Library (⌥↵: 24 generated gradient photos with no EXIF or places, written
  to Application Support and opened as an ordinary library), plus a Use
  Pictures Folder link (⇧↵). Each shows its shortcut. The copy explains local-first
  storage: Lumina works on photos where they are or copies them in, never
  changes the originals, and keeps its catalog in Application Support.
- Recent libraries on the welcome screen (keyboard: ↑/↓, ↵, ⌫) and in ⌘K
  (Open Recent Library, Clear Recent Libraries). Forgetting one never deletes
  its folder or catalog. A missing last library or recent is named in a notice
  instead of being skipped silently.
- No guided setup or import options on the welcome screen.

**Target**

- Three paths: open existing library, import/copy into a new library, explore a
  sample library.
- Explain local-first storage and where files live.
- Import progress with cancel/retry; duplicate preview; permission fallback.
- Recent libraries on launch.

**Acceptance:** a new user reaches a populated library without reading docs;
canceling import leaves no corruption; every failure has a recovery action.

## 10. Personalization and access

**Current**

- Dark-only theme; no accent choice; no motion settings; no shortcut customization.
- Preferences: General, Storage, About only.
- No tabs, multi-window, recent views or in-app updates.
- GPUI exposes no NSAccessibility tree; VoiceOver sees only the window. Keyboard
  coverage is good but tab order and focus rings are incomplete. English only.

**Target**

- Light/system theme, accent choice, reduced-motion setting, density option.
- Preference panes for appearance, import, video, shortcuts, backup, diagnostics.
- Custom keyboard shortcuts with conflict detection.
- Tabs/multi-window and recent views.
- Accessibility bridge, focus rings, tab order, localization.

**Acceptance:** every core action is keyboard reachable; reduced motion is
honored; themes do not regress contrast; accessibility limitations are documented.

## Recommended UX passes

### UX-1: Feel and orientation

Target: 0.3. Low risk, high perceived quality.

- Selection/open semantics, empty-space clear, hover/focus/press states.
- Viewer shared-element open/close and item crossfade.
- Motion tokens and reduced-motion hook.
- Skeleton cards, thumbnail fade-in, actionable empty states.
- Toasts with undo, non-blocking export/import jobs, watcher summary.
- Back, restored last view/search/selection/scroll, recent libraries.

### UX-2: Precision browsing

Target: 0.4.

- Pointer/keyboard-anchored grid zoom with animated reflow.
- Viewer cursor-anchored zoom, pinch, double-click fit/100%, presets and HUD.
- Jump-to-date.
- Video drag scrubbing with real frame previews, buffered bar, resume position,
  auto-hide HUD, persisted speed/volume.
- Search suggestions/history and query explanation.
- Untagged, tag context menu, smart-album previews/confirmation.
- Mixed-value batch editing with grouped undo.

### UX-3: Immersion

Target: 0.6.

- Compare, slideshow, map.
- Viewer quick rotate/crop and external editor handoff.
- Video chapters, audio/subtitle tracks, loop range, trim later.
- Animated transitions between viewer items and fullscreen.

### UX-4: Personalization and access

Target: 0.7.

- Light/system theme, accent, density, reduced motion, motion tuning.
- Shortcut customization, expanded Preferences, tabs/recent views.
- Accessibility bridge/focus order, localization, contrast checks.

## Test and measurement plan

- UI automation: single vs double click, empty-space clear, shift/cmd selection,
  viewer open/close/return, viewer Back button, Escape chain, Back, restored
  view, zoom anchors, video scrub state.
- Visual checks: screenshot before/after for viewer transition, skeleton and
  empty states; no committed screenshots.
- Performance budgets: selection feedback under 16 ms; viewer transition
  160-240 ms; zoom/pan and video scrub at 60 FPS or better; no frame longer than
  16 ms during reflow.
- Manual QA: slow double click, rapid arrow navigation, trackpad pinch, reduced
  motion, fullscreen, PiP, interrupted import, failed thumbnail, empty library.

## Non-goals for this pass

- A full photo editor or RAW develop suite.
- Cloud photo libraries or remote browsing.
- Mobile/web clients.
- Decorative animation that delays input or ignores reduced motion.
