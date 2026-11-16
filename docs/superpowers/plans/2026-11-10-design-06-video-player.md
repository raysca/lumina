# Design 06: Video Player — Implementation Plan

> For agentic workers: REQUIRED SUB-SKILL: superpowers:subagent-driven-development or
> superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** Restyle the video viewer to the design: a title-bearing top bar, a larger framed
video, a full-width docked transport (chapter labels, thumbnail-strip scrubber, controls row)
and a video InfoPanel (Chapters, Playback, Location, Tags).
**Design:** `lumina.pen` frame `F6sXRs` "Video Player" (export with the pencil MCP to view).
**Icons (fetched in 00 Task 1):** `audio-lines` (also listed by 04), `share-2`, `panel-right` (also listed by 02). Full screen keeps the existing `maximize-2` (`icons.rs:120`) for the design's `maximize`. Already in `resources/icons/`: play, pause, rotate-ccw, rotate-cw, repeat, volume-2, volume-x, picture-in-picture-2, heart, star, chevron-left. Not needed while trim and captions are deferred: `scissors`, `captions`.
**Depends on:** Design 00 Foundation, and Design 02 Detail. Design 02 provides the docked
340 px InfoPanel, the 60 px top bar with 30 px action buttons, the panel section/row styles,
the Location card and the Tags chips; this plan reuses them.
**App today:**
- Viewer shell: `views/viewer/mod.rs:30-186`. Videos get the transport under the frame
  (`:137-159`) and a PiP hand-off pill when PiP owns the player (`stage_controls.rs:202-260`).
- Video frame size: `views/viewer/viewport.rs:34` (`VIDEO_FRAME_MAX_WIDTH = 1040`) and `:40-79`
  (`video_frame_size`, drawer and transport constants).
- Transport card: `views/viewer/transport.rs:88-590`. It is a floating card as wide as the
  frame (`:165-186`: `bg_app`, border, radius LG, p_3). Scrub row (`:188-353`): elapsed | 24 px
  track with played tint `rgba(0xF0B54B38)` (`:312`), loop band, chapter ticks, hover needle,
  playhead, time badge | total. Three control zones (`:371-590`):
  [mute · speed · audio] | [−5 s · step back · play · step forward · +5 s] |
  [loop · PiP · full screen]. Speed popover `:591`, audio menu `:648-740`.
- Scrub preview: `views/viewer/scrub_preview.rs:1-82` (160×90 frame above the track), fed by the
  strip `app/scrub_preview.rs:18-90` (`ScrubStrip.frames: Vec<(f64, RenderImage)>`, 256 px
  frames, built off-thread).
- Chapters and audio tracks: `app/video_tracks.rs:22-70` (from `player.chapters()`). ⌥←/⌥→ jump
  chapters (`app/keymap/viewer.rs:165-172`); plain ←/→ seek (`:200-207`, `seek_video_relative(±5)`).
- Player API: `player/backend.rs:19-59` (`set_volume` exists; no frame rate, codec, bitrate or
  subtitle API). `state.rs:91` `volume: f32` (0.85 default) has no UI.
- Panel for a video: `views/inspector/summary.rs:195-204` (stat strip: duration and resolution
  only; comment "no fps cell; frame rate is not in the catalog yet").

## Global Constraints
- AGENTS.md is normative: debug_selector on controls, keyboard path for every action,
  command palette entry for global actions, no UI blocking on IO, tests per change.
- UI copy never names yt-dlp, ffmpeg, curl, brew or HLS.
- Design copy/numbers are sample data ("MVI_0412.MOV", "Departure", "142 Mb/s", "Mara"):
  render real values.
- Colours/fonts only through Foundation tokens; no new hex literals in views. Replace the
  existing `rgba(0xF0B54B38)` (`transport.rs:312`). Player black `#060608` is
  `theme::bg_stage()`; strip dimming and pills use `media_scrim()`, gradients `media_fade()`,
  floating chrome `bg_float()`. Never `bg_viewer`/`bg_hud` (00 deletes them). `#FFFFFFD9` ticks
  and `#1A1206` on-accent text: ask 00 for named tokens if it lacks them.
- New user-facing strings go through `src/i18n.rs`.
- Existing selectors `video-transport`, `video-scrubber`, `video-play`, `video-speed`,
  `video-mute`, `video-loop-range`, `video-chapter-tick-N` and `video-played` are a test
  contract (`tests/video_player.rs`, `video_scrub.rs`, `video_tracks.rs`). Keep the names on
  the new elements.

## Current vs design
| Element | App today (file:line) | Design (exact values) | Change |
|---|---|---|---|
| Background | `bg_viewer()` (`viewer/mod.rs:54-58`), deleted by 00 | `#060608` → `theme::bg_stage()` | restyle (token) |
| Back | "Library" (`top_bar.rs:126-162`) | "Videos" (source view), Design 02 style | copy (Design 02 Task 2) |
| Title + meta | Centre date + "(n of m)" (`top_bar.rs:65-97`) | Left, after Back with gap 14: title Inter Tight 15/600 −0.2 `t1` (display title), meta JetBrains Mono 10 `t3` "14 Jun 2025 · 4K HDR". No position counter | relayout + copy (video only) |
| Actions | Favourite, Info (`top_bar.rs:169-229`) | heart, star, scissors, share-2, picture-in-picture-2, panel-right (active) | new: PiP button calls `play_in_pip` (moves from the transport). Scissors is not in the app (see decisions) |
| Video frame | ≤1040 wide, contain-fit (`viewport.rs:34,40-79`) | 1120×630 at 1600×1000 with panel open, radius 3, shadow 0 16 48 `#000000CC` | restyle; recompute `video_frame_size` for the docked transport (≈205 h) and docked panel |
| HDR / fps badge | none | Top-left 16/16, h 20, padding 0 7, radius 4, scrim, JetBrains Mono 9.5 `accent` "HDR · 50 fps" | not in app (see decisions) |
| Caption overlay | none | Bottom-centre box 400×30, scrim `#000000B3`, radius 5, Inter 13 `#E8E8EF` | not in app (see decisions) |
| Transport container | Floating card, frame-wide, rounded (`transport.rs:165-186`) | Full stage width (1260), border-top 1 `line`, padding 16 28, gap 11, no radius | relayout |
| Chapter labels | Ticks only (`transport.rs:292-301`) | Row h 15: "Name m:ss" JetBrains Mono 9.5 at each chapter's x. Current chapter `accent`, others `t3` | new (data exists) |
| Scrubber | 24 px tinted track (`transport.rs:199-351`) | Thumbnail strip h 54, radius 3, 1 px gaps between frames. Frames after the playhead get a 42% dark overlay. Chapter ticks 1.5×54 white 85%. Playhead 2×68 `accent` (top −7), knob 10 `accent` (top −11), time pill below (h 19, radius 4, `accent` bg, mono 9.5 dark text) | restyle: strip from `ScrubStrip`; fall back to a plain `bg_2` bar until frames arrive |
| Loop range / hover needle / scrub preview | `transport.rs:280-291,315-326,348-350` | not shown | keep, drawn over the strip |
| Waveform | none | Row h 32, bars 8 wide gap 2, played `accent`, rest white 18% | not in app (see decisions) |
| Play | 32 px button in the centre zone (`transport.rs:476-506`) | Left group, gap 14: Play 38 round `accent` fill, dark icon | relayout + restyle |
| −5 s / +5 s | RotateCcw / RotateCw 32 px (`transport.rs:451,521`) | `rotate-ccw`, `rotate-cw` 30 round, after Play | relayout |
| Frame step buttons | StepBack / StepForward (`transport.rs:464,508`) | absent | remove buttons; keep , and . keys and palette (see below) |
| Time readout | Elapsed and total on either side of the track (`transport.rs:155-163,194,352`) | After the buttons: "0:34 / 1:18", JetBrains Mono 12, elapsed `t1`, "/" and total `t3` | relayout |
| Speed | 32 px button + popover (`transport.rs:409-450,591`) | Right group, gap 8: pill "1.0×" h 28, padding 0 10, radius 7, bg `bg_2`, outline `line`, mono 10.5 `t2` | restyle (popover unchanged) |
| CC | none | Pill "CC", `accent` text when on | not in app (see decisions) |
| Volume | Mute toggle only (`transport.rs:386-408`) | `volume-2` icon (mute toggle) + slider 62×4: track white 15%, level `t1` | new slider (data exists: `video.volume`, `set_volume`) |
| Audio track button | Shown when the file offers a choice (`transport.rs:648`) | absent | keep (conditional) |
| Loop | Repeat 32 (`transport.rs:543`) | `repeat` 30 round | restyle |
| PiP button | In the transport (`transport.rs:556`) | Moved to the top bar | relayout |
| Full screen | Maximize 32 (`transport.rs:572`) | `maximize` 30 round | restyle |
| Panel header | Title, filename, date (`summary.rs:51-80`) | Design 02 header. Badge = resolution label ("4K"); When line has seconds + size | reuse Design 02 |
| Panel: Scenes | none | "SCENES" + "Re-detect" (`accent`). Rows h 44, radius 7, bg `bg_0`, outline `line`, padding 0 8, gap 10: thumb 50×28 radius 3, name Inter 12 `t1`, range mono 9.5 `t3` "0:22 — 0:48". Current row: bg `bg_3`, outline `accent`, name `accent` 500, `audio-lines` icon right | new: list `video.tracks.chapters`, thumbs from the strip; click seeks |
| Panel: Playback | Stat strip duration + resolution (`summary.rs:195-204`) | Rows: Duration "1:18.420", Resolution "3840 × 2160", Frame rate, Codec, Bitrate, Audio | relayout. Duration and Resolution now; others see decisions |
| Panel: Location / People / Tags | as in Design 02 | as in Design 02 | reuse Design 02 (People: dropped there) |

## Not in the app yet (needs a product decision)
- **Trim (scissors)** (L). There is no trim UI or export of a cut range. The loop in/out points
  (`app/loop_range.rs`) could seed a range, but writing a trimmed file needs a new export path.
  **Recommend: separate plan. Omit the button until then.** Do not ship a dead control.
- **Captions / CC** (L, player). The backend has no subtitle API. It needs the AVFoundation
  legible media selection group in `player/macos.rs`, an overlay renderer and a menu.
  **Recommend: separate plan. Omit the CC pill and the overlay until then.**
- **Waveform** (L, core). The app does not extract audio amplitude. It needs an off-thread audio
  decode (AVAssetReader on macOS; never the network-import tools) with a cached result.
  **Recommend: drop from the design for now. Revisit with the trim plan, which benefits most
  from it.**
- **HDR / fps badge and Playback rows Frame rate, Codec, Bitrate, Audio** (M, player). The
  catalog lacks them (`summary.rs:197` note). AVFoundation can report them at load
  (`nominalFrameRate`, format descriptions, `estimatedDataRate`, audio channel layout and sample
  rate). **Recommend: separate plan that adds a `VideoInfo` read from the loaded player (no
  schema change). This plan shows only the rows it knows and hides the badge.**
- **"Re-detect" scenes** (L). Chapters come from the file. The app has no scene detection.
  **Recommend: title the section "Chapters" (or keep "Scenes" if design insists), show it only
  when the file has chapters, and drop "Re-detect".**

## App has, design lacks
- Frame-step buttons (`transport.rs:464,508`). Remove the buttons and keep the , / . keys and
  palette commands. Ask design whether to keep them in a "…" menu.
- Audio track menu (`transport.rs:648-740`). Keep: shown only for multi-track files.
- Loop range band, hover needle, scrub preview card (`transport.rs:280-350`,
  `scrub_preview.rs`). Keep.
- Chapter jump keys ⌥←/⌥→ (`keymap/viewer.rs:165-172`) and plain ←/→ seek. Keep.
- PiP hand-off pill (`stage_controls.rs:202-260`). Keep. It replaces the docked transport while
  PiP owns the player.
- Position "(n of m)" (`top_bar.rs:86-97`). The frame drops it for video. Keep it out of the
  top bar; [ and ] still step.
- Immersive full screen with HUD fade (`viewer/mod.rs:143-154`). Keep: the docked transport
  fades with the HUD there.

## Review Focus
- **No strip yet / strip failed:** the scrubber shows a plain `bg_2` bar with the same hit area
  and playhead. Seeking still works, and the Scenes thumbs fall back to a neutral box.
- **No chapters:** no label row (the transport shrinks by 15 + gap), no Scenes section, and ⌥←/⌥→
  seek 5 s (`app/video_tracks.rs:124`).
- **Many or close chapters:** labels that would overlap are dropped, but the current chapter's
  label is always kept. Scenes list scrolls with the panel.
- **Very short (<2 s) or very long (2 h) clips:** time formats switch to h:mm:ss. Strip frame
  count stays bounded by the strip builder.
- **Keyboard:** Tab order runs play → −5 s → +5 s → speed → mute → volume → loop → full screen.
  The volume slider needs a keyboard path: ↑/↓ while the slider is focused, plus palette
  "Volume Up/Down". Check that ↑/↓ are not already bound in `keymap/viewer.rs` before binding
  them globally.

## Tasks

### Task 1: Video top bar
**Files:** Modify `views/viewer/top_bar.rs` (after Design 02 Task 2) / Test
`tests/video_player.rs`
- [ ] Write failing test `video_top_bar_shows_title_meta_and_pip`. For an open video:
      `viewer-title` = display title, `viewer-meta` contains the date and the resolution label,
      `viewer-position` absent. Clicking `viewer-pip-btn` sets `video.in_pip`. No
      `viewer-trim-btn`.
- [ ] Implement: left group (Back, title Inter Tight 15/600, meta mono 10 `t3`); actions heart,
      star, share, PiP, info. The PiP tooltip names its key (⌥P, `keymap/viewer.rs:117`).
- [ ] Run `cargo test -p lumina-ui-gpui video_player`; expect pass.
- [ ] Gate (fmt, clippy -D warnings, full workspace tests, `make ui-smoke`) and commit
      `feat(design): video viewer top bar` staging explicit paths.

### Task 2: Docked transport layout and controls row
**Files:** Modify `views/viewer/transport.rs:83-590`, `views/viewer/viewport.rs:34-79`,
`views/viewer/mod.rs:137-159` (split into `transport.rs` + `transport_controls.rs` to stay under
~500 lines) / Test `tests/video_player.rs`
- [ ] Write failing test `video_transport_is_docked_full_width`. At 1600×1000 with the panel
      open: `video-transport` left 0, width 1260, bottom 1000. `video-play` is 38×38 and left of
      `video-seek-back` and `video-seek-fwd` (existing selectors, kept). `video-time` text
      matches "m:ss / m:ss". `video-step-fwd` and `video-step-back` are absent (today they
      exist, so this fails before the change). The video frame width is ≥1100. Update the existing
      tests that assumed the card or the frame-wide max width.
- [ ] Implement: border-top `line`, padding 16 28, gap 11, full stage width. Controls row is
      40 h. Left: Play 38 `accent` fill, ±5 s 30 round, time mono 12. Right: speed pill, mute +
      volume slider, (audio), loop, full screen. Raise `VIDEO_FRAME_MAX_WIDTH` to 1120 and
      update `TRANSPORT_BLOCK`. Drop the drawer constant (the panel is docked).
- [ ] Run `cargo test -p lumina-ui-gpui video`; expect pass.
- [ ] Gate and commit `feat(design): docked video transport`.

### Task 3: Thumbnail-strip scrubber with chapter labels
**Files:** Modify `views/viewer/transport.rs:188-353`, `views/viewer/scrub_preview.rs` / Test
`tests/video_scrub.rs`, `tests/video_tracks.rs`
- [ ] Write failing tests. `scrubber_is_a_strip_with_playhead_pill`: `video-scrubber` is 54 h;
      with a strip present `video-strip-frame-0` exists; `video-playhead-time` text equals the
      current time. `chapter_labels_mark_the_current_chapter`: with 3 chapters,
      `video-chapter-label-2` exists and is flagged current at t inside chapter 2. Existing
      drag/seek tests in `video_scrub.rs` must keep passing unchanged.
- [ ] Implement: frames tile the track (nearest strip frame per slot, 1 px gaps, radius 3).
      Frames after the playhead get a 42% scrim. Ticks are 1.5 px. Playhead 2×68 + knob 10, with
      the time pill under the strip. Keep the same `on_children_prepainted` bounds capture so
      `scrub_fraction` mapping is unchanged. Chapter label row h 15: drop labels that would
      overlap, always keeping the current one.
- [ ] Run `cargo test -p lumina-ui-gpui video_scrub` and `video_tracks`; expect pass.
- [ ] Gate and commit `feat(design): thumbnail-strip video scrubber`.

### Task 4: Volume slider
**Files:** Modify `views/viewer/transport.rs` (mute zone), `app/video_actions.rs` (new
`set_video_volume` beside `toggle_video_mute` `:546` and `set_video_speed` `:562`; `:179` is
only the load-time `player.set_volume` call, which should read the restored value),
`src/config.rs` (new `video_volume: Option<f32>` beside `video_speed`/`video_muted`
`:416-419`), `app/command_palette.rs`, `src/i18n.rs` / Test `tests/video_player.rs`,
`src/config.rs` tests
- [ ] Write failing tests. UI `volume_slider_sets_player_volume`: clicking at 25% of
      `video-volume` sets `video.volume` ≈ 0.25 and the player volume; palette "Volume Up"
      raises it by 0.1, clamped to 1.0. Config `config_video_volume_roundtrip_and_default`
      (temp path, like `config_video_speed_and_mute_roundtrip_and_default` `config.rs:1177`):
      0.4 round-trips; a missing key restores 0.85; NaN or out-of-range clamps.
- [ ] Implement: `set_video_volume(v)` clamps to 0..=1, updates `video.volume`, calls
      `player.set_volume`, and saves config the way speed and mute do (`save_config`, `app/state.rs:710`; its `#[cfg(test)]`
      `save_config` early return keeps tests from writing real config; a save error is
      surfaced, not swallowed). Slider: 62×4 track (white 15%), level `t1`, drag via
      `drag_tracker`, focusable with ↑/↓ while focused.
- [ ] Run `cargo test -p lumina-ui-gpui video_player`; expect pass.
- [ ] Gate and commit `feat(design): video volume slider`.

### Task 5: Video InfoPanel (Chapters, Playback)
**Files:** Modify `views/inspector/summary.rs:195-242` (split a `summary_video.rs` if needed) /
Test `tests/inspector.rs`
- [ ] Write failing test `video_panel_lists_chapters_and_playback`. For a video with 3 chapters:
      `inspector-chapter-1..3` exist. Clicking `inspector-chapter-2` seeks `playback_time` to
      its start. `inspector-playback-duration` shows milliseconds ("1:18.420" form) and
      `inspector-playback-resolution` "W × H". There are no Frame rate / Codec rows. Without
      chapters there is no chapter section.
- [ ] Implement: chapter rows (h 44, radius 7, `bg_0` + `line` outline; current row `bg_3` +
      `accent` outline, `accent` 500 name, `audio-lines` icon). Thumbs 50×28 come from
      `preview_strip` nearest each start. Range is "start — end" mono 9.5. Playback rows use the
      Design 02 key/value style. The badge shows the resolution label.
- [ ] Run `cargo test -p lumina-ui-gpui inspector`; expect pass.
- [ ] Gate (include `make ui-smoke`) and commit `feat(design): video info panel`.

Manual visual checks (compare with `F6sXRs.png`): strip dimming, tick colour, playhead pill
contrast, Play fill and icon colour, frame shadow.

## Decisions (2026-11-12: recommendations accepted)
- Trim, Captions/CC, Waveform, the HDR/fps badge and the Frame rate/Codec/Bitrate/Audio rows are left out of this plan and go to separate plans. No dead controls.
- Scenes: the section is titled "Chapters", appears only when the file has chapters, and has no "Re-detect".
- The frame-step buttons are removed. The , and . keys and their palette commands stay.
