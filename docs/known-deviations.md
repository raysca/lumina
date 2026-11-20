# Known deviations

Every ponytail note in the tree, indexed. These are deliberate, documented
shortcuts: each says what is not done and why. Fixing one means deleting its
note in the same change; do not add a note for something the current task
requires doing.

Regenerate with:

    grep -rn ponytail crates --include="*.rs"

Total: 120 notes.


## crates/lumina-core/src/cache.rs

- L234: ponytail: each trim stats the whole cache dir (~every 190 MB written);

## crates/lumina-core/src/catalog/albums.rs

- L104: ponytail: O(memberships of albums without a key photo) per call, once per
- L388: ponytail: the sort is still per page, O(members); a big album needs

## crates/lumina-core/src/catalog/dims.rs

- L34: ponytail: a restart re-measures from the first row; keep a resume

## crates/lumina-core/src/catalog/edits.rs

- L182: serves it. ponytail: cost grows with the number of edited items;

## crates/lumina-core/src/catalog/keyword_ops.rs

- L193: ponytail: a list whose every member was deleted stays as

## crates/lumina-core/src/catalog/media.rs

- L26: ponytail: undoing an edit keeps its flag, so that item then ignores
- L298: ponytail: one whose `moov` lies outside the scanned head/tail windows is

## crates/lumina-core/src/catalog/mod.rs

- L132: ponytail: one trivial query per unlock (~µs); a write hook would

## crates/lumina-core/src/catalog/search.rs

- L71: ponytail: max(rowid) stands in for the row count (O(1)); it

## crates/lumina-core/src/catalog/tags.rs

- L325: ponytail: even with LIMIT 24 every match is read and sorted, so the cost

## crates/lumina-core/src/decoder.rs

- L316: ponytail: a process that finishes exiting between `spawn` and this
- L641: ponytail: a sips process plus a PNG round trip costs about 50 ms per

## crates/lumina-core/src/engine/sidecars.rs

- L109: ponytail: read-then-write per field, so an edit landing in the

## crates/lumina-core/src/export.rs

- L192: ponytail: decode bypasses the thumbnail disk cache, so a large export

## crates/lumina-core/src/import.rs

- L353: ponytail: volumes without hard links (exFAT, SMB) fall back to

## crates/lumina-core/src/lib.rs

- L296: ponytail: one banner per engine; later failures only log, since

## crates/lumina-core/src/scanner.rs

- L65: ponytail: a second directory walk (no file reads); drop it if a huge

## crates/lumina-core/src/search/explain.rs

- L76: ponytail: hundredths heuristic; an explicit `f:>=2.81` reads

## crates/lumina-core/src/search/mod.rs

- L275: ponytail: the sort is of every match, and `-is:edited` builds the
- L369: ponytail: a very rare term walks the whole timeline index;

## crates/lumina-core/src/search/parse.rs

- L62: ponytail: unknown prefixes are text when positive, so negated ones are excluded text

## crates/lumina-core/src/smart_albums/rules_sql.rs

- L466: ponytail: a month is 30 days; calendar months if anyone notices.

## crates/lumina-core/src/smart_albums/system.rs

- L85: ponytail: no index on added_epoch_ms, so this scans like Places and
- L91: ponytail: every page sorts all downloads, fine for URL imports (past the

## crates/lumina-core/src/stacks/mod.rs

- L121: ponytail: name only, since the catalog keeps no copy marker. A renamed
- L162: ponytail: both cameras must be known and equal, so RAW formats the

## crates/lumina-core/src/text.rs

- L31: ponytail: no Unicode normalisation (NFC), which needs a new dependency;
- L80: ponytail: Latin-1 and Latin Extended-A letters plus Romanian ș/ț only;

## crates/lumina-core/src/update/install.rs

- L269: ponytail: nothing more to try; the old app is at `backup`.
- L296: ponytail: runs when the next update is staged; a launch-time sweep if

## crates/lumina-core/src/xmp/mod.rs

- L90: ponytail: checked before the rename, so a sidecar another app writes

## crates/lumina-core/tests/library_tests.rs

- L785: ponytail: fixtures stay at or below 2560 px; larger JPEGs decode through

## crates/lumina-core/tests/watcher_test.rs

- L367: ponytail: the lib's decoder test helper is crate-private, unreachable here.

## crates/lumina-ui-gpui/src/app/active_view.rs

- L110: ponytail: one engine query for All Media too (two index seeks), not
- L261: ponytail: rows × batch scan; a watcher batch is a few files.
- L493: ponytail: no throttle, so a long scan refetches back to back; add one
- L518: ponytail: a panicking fetch would leave this set and freeze the

## crates/lumina-ui-gpui/src/app/albums.rs

- L366: ponytail: not on the undo stack; pinning again is the undo.

## crates/lumina-ui-gpui/src/app/batch_summary.rs

- L97: ponytail: each marquee step or Shift+arrow over a large range asks

## crates/lumina-ui-gpui/src/app/downloads/job.rs

- L76: ponytail: outer message only; inner causes can name the

## crates/lumina-ui-gpui/src/app/downloads/panel.rs

- L349: ponytail: walks the album like the startup restore does; an

## crates/lumina-ui-gpui/src/app/downloads/pause.rs

- L45: ponytail: a placeholder (insert not back yet) isn't paused; it

## crates/lumina-ui-gpui/src/app/import.rs

- L339: ponytail: failures only in the banner (it says X of Y); the

## crates/lumina-ui-gpui/src/app/jobs.rs

- L65: ponytail: no cancel control yet; the engine checks this between files.

## crates/lumina-ui-gpui/src/app/library.rs

- L180: ponytail: one library's points at a time; switching back starts

## crates/lumina-ui-gpui/src/app/missing.rs

- L38: ponytail: a poll, only while the bar names a drive; a mount notification

## crates/lumina-ui-gpui/src/app/mosaic_layout.rs

- L243: ponytail: a panicking build would leave this set and freeze the

## crates/lumina-ui-gpui/src/app/network_import.rs

- L69: ponytail: no clipboard read on open; macOS may prompt for every
- L182: ponytail: inspecting holds an executor worker for up to the
- L219: ponytail: outer message only; inner causes can name

## crates/lumina-ui-gpui/src/app/present.rs

- L304: ponytail: writes with no history entry are not seen: permanent
- L371: ponytail: one primary-key catalog read on the UI thread per mark key

## crates/lumina-ui-gpui/src/app/render/empty_state.rs

- L303: ponytail: no per-term counts or kind icons.

## crates/lumina-ui-gpui/src/app/render/empty_state/parts.rs

- L43: ponytail: the frame's radial glow as a soft shadow, by eye.

## crates/lumina-ui-gpui/src/app/render/grid.rs

- L50: ponytail: the band shows the month recorded by the previous
- L262: showing the top row's month. ponytail: one sticky band, not a header

## crates/lumina-ui-gpui/src/app/render/mod.rs

- L39: ponytail: the design's -0.2 tracking is skipped; GPUI has no

## crates/lumina-ui-gpui/src/app/render/mosaic.rs

- L262: ponytail: the design's -0.2 tracking is skipped; GPUI

## crates/lumina-ui-gpui/src/app/render/toolbar.rs

- L346: ponytail: a full circle can't be one arc; stop just short.

## crates/lumina-ui-gpui/src/app/restore.rs

- L42: ponytail: a queue still loading counts as not empty, so the

## crates/lumina-ui-gpui/src/app/scrub_preview.rs

- L124: ponytail: no banner; the preview is an enhancement and the

## crates/lumina-ui-gpui/src/app/search/top_matches.rs

- L306: ponytail: albums and tags are not in the key; a rename shows on

## crates/lumina-ui-gpui/src/app/selection_by_id.rs

- L110: ponytail: one inexact case. A rating while sorted by Rating can move

## crates/lumina-ui-gpui/src/app/slideshow.rs

- L183: ponytail: the net count change is a lower bound. Rows both
- L212: ponytail: a wide selection is cleared after any catalog write during

## crates/lumina-ui-gpui/src/app/smart_cards.rs

- L210: ponytail: O(items) sums per call, a few hundred additions on a

## crates/lumina-ui-gpui/src/app/smart_draft/labels.rs

- L102: ponytail: linear search, the library has a handful of cameras.

## crates/lumina-ui-gpui/src/app/stacks.rs

- L335: ponytail: one small sync read of this stack's rows on the UI

## crates/lumina-ui-gpui/src/app/tag_merge.rs

- L34: ponytail: O(keywords) per frame, in memory, like the list filter.

## crates/lumina-ui-gpui/src/app/tag_undo.rs

- L68: ponytail: clears even when this view's items did not change;

## crates/lumina-ui-gpui/src/app/updates.rs

- L168: ponytail: the download holds an executor worker for its length;

## crates/lumina-ui-gpui/src/app/video_actions.rs

- L223: what the player knows. ponytail: session only.
- L282: ponytail: one stat on the UI thread, where the load already

## crates/lumina-ui-gpui/src/app/viewer_actions.rs

- L262: ponytail: no timeout; on Linux/Windows the editor runs directly and

## crates/lumina-ui-gpui/src/i18n/en_us.rs

- L57: ponytail: chrono's %b is always the English month (no locale

## crates/lumina-ui-gpui/src/player/backend.rs

- L25: ponytail: the default assumes 30 fps; the macOS player overrides it

## crates/lumina-ui-gpui/src/tests/delete_dialog.rs

- L227: ponytail: not confirmed here, it would move a file to the real ~/.Trash.

## crates/lumina-ui-gpui/src/tests/downloads_sections.rs

- L556: ponytail: `click` runs until parked, so the background removal has

## crates/lumina-ui-gpui/src/tests/get_info.rs

- L435: ponytail: GPUI exposes no transform to assert on, so the heart's pop

## crates/lumina-ui-gpui/src/views/album_builder/mod.rs

- L54: ponytail: the frame has no border or shadow; line-2 and

## crates/lumina-ui-gpui/src/views/album_page/header.rs

- L228: ponytail: the design's -0.6 tracking is skipped;

## crates/lumina-ui-gpui/src/views/albums_overview/albums.rs

- L33: ponytail: a 200 minimum gives 6 cards of 201 at a 1600 window;

## crates/lumina-ui-gpui/src/views/albums_overview/smart.rs

- L104: ponytail: not virtualised (a library has a handful of smart albums);

## crates/lumina-ui-gpui/src/views/downloads_filter_bar.rs

- L72: ponytail: no speed cap yet.

## crates/lumina-ui-gpui/src/views/downloads_popover.rs

- L98: ponytail: the design's -0.2 tracking is skipped; GPUI has none.

## crates/lumina-ui-gpui/src/views/downloads_popover_rows.rs

- L292: ponytail: no size; the row stores none. Add it if the

## crates/lumina-ui-gpui/src/views/duplicates/card.rs

- L27: ponytail: the facts' height is a measured constant, not laid out; the

## crates/lumina-ui-gpui/src/views/duplicates/kinds.rs

- L65: ponytail: one sort exists, so a label and no sort button.

## crates/lumina-ui-gpui/src/views/duplicates/list.rs

- L51: ponytail: the design's +0.9 tracking is skipped; GPUI

## crates/lumina-ui-gpui/src/views/floating_pip.rs

- L382: ponytail: GPUI has no letter-spacing; the design's

## crates/lumina-ui-gpui/src/views/grid.rs

- L50: meta pill hide. ponytail: under ~86 px (8 columns, narrow window, inspector
- L485: ponytail: assumes the Library right padding (20); an album row has 24, so
- L655: ponytail: a fixed overdraw of about one viewport; the window

## crates/lumina-ui-gpui/src/views/inspector/batch.rs

- L231: ponytail: one map lookup per selected id each frame; cache in the

## crates/lumina-ui-gpui/src/views/inspector/body.rs

- L358: ponytail: "Open With ▾" needs a native app-list

## crates/lumina-ui-gpui/src/views/inspector/location_card.rs

- L64: ponytail: the land is clipped to the card's rectangle, not its 6 px
- L189: ponytail: one entry, the item on show; the panel shows one item.

## crates/lumina-ui-gpui/src/views/inspector/summary.rs

- L251: ponytail: no letter spacing (design: 1); GPUI text has no tracking.
- L352: ponytail: no neighbourhood or country names; the offline geocoder only

## crates/lumina-ui-gpui/src/views/inspector/tabs/location.rs

- L29: ponytail: "nearby" is the same city via the existing city: search; a

## crates/lumina-ui-gpui/src/views/inspector/tabs/tags.rs

- L75: ponytail: a linear alias scan per unmatched keyword; index aliases by

## crates/lumina-ui-gpui/src/views/missing_library.rs

- L189: ponytail: the error shows directly, no "Show details"

## crates/lumina-ui-gpui/src/views/network_import_dialog.rs

- L192: ponytail: a static line, not a spinner; `motion` has no
- L266: ponytail: the design's -0.3 tracking is skipped; GPUI 0.3.11 has

## crates/lumina-ui-gpui/src/views/network_import_preview.rs

- L78: ponytail: no remote thumbnail; a film placeholder.

## crates/lumina-ui-gpui/src/views/onboarding.rs

- L112: ponytail: the design's -0.6 tracking is skipped; GPUI

## crates/lumina-ui-gpui/src/views/page_header.rs

- L59: ponytail: the design's -0.3 tracking is skipped; GPUI

## crates/lumina-ui-gpui/src/views/preferences/storage_card.rs

- L29: ponytail: the design names no chart colours; the label blue and

## crates/lumina-ui-gpui/src/views/results_head.rs

- L63: ponytail: the design's -0.2 tracking is skipped; GPUI

## crates/lumina-ui-gpui/src/views/search_bar.rs

- L217: ponytail: no clock icon in the set; rotate-ccw reads as history.

## crates/lumina-ui-gpui/src/views/sidebar/dormant.rs

- L68: ponytail: GPUI 0.3.11 has no aria disabled; the

## crates/lumina-ui-gpui/src/views/tag_manager/detail.rs

- L238: ponytail: O(keywords) per frame, in memory; cache with the counts

## crates/lumina-ui-gpui/src/views/tag_manager/detail_sections.rs

- L238: ponytail: a window of 8 around the highlight, no virtual list;

## crates/lumina-ui-gpui/src/views/toast.rs

- L26: ponytail: no Danger kind; write failures go to the banner (AGENTS.md §2).
- L61: ponytail: View/Retry from the design land with their first caller.

## crates/lumina-ui-gpui/src/views/viewer/chapter_labels.rs

- L31: ponytail: widths are estimated from the mono advance, not shaped text;

## crates/lumina-ui-gpui/src/views/viewer/playback_failed.rs

- L121: ponytail: the error shows directly under the card, no "Show

## crates/lumina-ui-gpui/src/views/viewer/transport.rs

- L83: ponytail: width from the mono advance (9.5px * 0.6) plus padding.

## crates/lumina-ui-gpui/src/views/viewer/viewport.rs

- L148: ponytail: the inset stays while zoomed, so zoom geometry keeps one
