# Known issues

Open bugs, work that still runs on the UI thread or has not been measured,
flaky tests and tooling gaps. Remove an entry when it is fixed; add one when
you find something you are not fixing now. Planned features live in
[ROADMAP.md](ROADMAP.md).

## Bugs

### Playback and downloads

- The guard that makes loop and mute do nothing on a clip that failed to load
  lives in the callers (palette, PiP keys). It belongs in `toggle_video_loop`
  and `toggle_video_mute` (`crates/lumina-ui-gpui/src/app/video_actions.rs`)
  so every caller gets it.
- The downloads popover's "N active" pill counts only running and waiting rows,
  not paused ones.
- The download speed graph keeps its last bar when every job stalls: a stalled
  job's speed drops to zero, but no new sample is added.
- Plain (non-page) downloads report an average speed every 100 ms
  (`crates/lumina-core/src/net_import/direct.rs`), so their speed never goes
  stale even when the transfer stalls.

### Settings

- Reopening Settings while a thumbnail-cache clear is waiting for its final
  size reading can show "Cleared" again (`crates/lumina-ui-gpui/src/app/preferences.rs`).
- "Preferences" and "Settings" name the same page: the menu item, the ⌘K
  command, the shortcut sheet's ⌘, row and the downloads settings buttons say
  Preferences, while ⌘K "Settings: Downloads", "Reset Settings to Defaults…"
  and the shortcut sheet's navigation rows say Settings.

### Organizing

- Undoing a duplicate merge puts the trashed copies back, but not the metadata
  that was merged onto the keeper.
- In Duplicates, the ORIGINAL label and the group title follow the automatic
  pick, not the copy chosen as keeper; "ORIGINAL" is not always the earliest
  copy.
- Deleting a keyword leaves it in smart-album "include any of" lists, where it
  then matches nothing.
- An empty album shows an "Add from Library" button even when its title lookup
  fails, but ⌘K offers the command only when it succeeds.
- The error banner's detail can repeat the title in other words, for example
  "Could not create album" above "creating an album: database is locked".

### Places, dates and search

- Cameras that write an empty GPS block as 0,0 get a map pin in the Gulf of
  Guinea, in the location card and on Places. The scanner should drop 0,0.
- Neither the location card nor the Places map wraps across the date line.
- City search is accent-sensitive: "sao" does not find "São Paulo" (SQLite
  `LIKE` folds ASCII case only).
- Items with a capture time of 0 are filed under January 1970 in the month band
  (and the 1970 bucket of the timeline rail) instead of "Undated".
- The Videos header understates the total running time: clips whose duration
  has not been read add their size but not their length, with no hint.

### Photos on Linux

- Large TIFFs whose orientation tag sits past the first 8 MB show unrotated: the
  orientation read stops there (`crates/lumina-core/src/decoder.rs`).

### Accessibility

- Text set through the macOS Accessibility API (`AXSelectedText`) does not reach
  Lumina's text fields.
- With the viewer open, the toolbar and grid stay in the accessibility tree
  under it (the sidebar is already removed).

## Performance (work on the UI thread, or not measured)

- The smart-album builder runs its count and preview queries on the UI thread
  after every edit. Measured once at about 5.6 ms per edit on a 25k library
  (release); there is no bench gate.
- The Settings storage card's query (about 22 ms at 105k rows, release, measured
  once) runs off the UI thread but holds the catalog lock while it scans. No
  bench gate.
- Merge All in Duplicates runs its transaction on the UI thread.
- Opening a library opens the engine on the UI thread (only Retry on the
  welcome screen opens it in the background).
- Duplicates filters every group several times per frame (about 50k hash probes
  at 10k groups). Not measured.
- The library keeps laying out, refreshing and loading thumbnails under the
  Settings page. Not measured.

## Flaky tests

These passed on re-run; each failed only under heavy machine load (for example
while another `cargo test` was running).

- `tests::downloads_pause::pause_download_stops_job_and_keeps_row` (download
  queue timing).
- `views::mosaic::tests::justify_100k_within_budget` (took 199 ms against its
  60 ms debug budget at a load average around 22).
- `tests::pip::pip_window_takes_the_player_keys`.

## Tooling

- `scripts/live_ui_automation.sh` fails with "could not capture the window" when
  Screen Recording is not granted, before its Accessibility check can report
  SKIP. AGENTS.md §6.4 says a missing permission skips; a capture failure should
  map to SKIP unless `LUMINA_REQUIRE_UI_PERMISSIONS=1`.
