//! Slideshow: ⌥⌘S / View > Slideshow / ⌘K play the current view
//! in the immersive viewer on a GPUI timer. Space pauses, ←/→ step, one Esc
//! leaves the slideshow and the viewer and puts the selection back, every
//! other key except ⌘Q and ⌘, is absorbed, videos show their poster, the end
//! stops unless Loop is on, and Reduce Motion drops the slide fade.

use std::time::Duration;

use crate::tests::culling::{card, flags, ids};
use crate::tests::helpers::{app_with_library, click, keystrokes, render_ui, seed_library};
use crate::views::sidebar::AlbumSelection;
use crate::*;
#[cfg(target_os = "macos")]
use lumina_core::{LuminaEngine, MediaType};

const INTERVAL: Duration = Duration::from_secs(5);

fn idle(vcx: &mut VisualTestContext, by: Duration) {
    vcx.executor().advance_clock(by);
    vcx.run_until_parked();
    render_ui(vcx);
}

fn shown(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<usize> {
    view.update(vcx, |app, _| app.selected_index)
}

fn playing(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.slideshow.is_some())
}

fn paused(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| {
        app.slideshow
            .as_ref()
            .expect("the slideshow is running")
            .paused
    })
}

/// Clicks card `index` and starts the slideshow with ⌥⌘S.
fn start_at(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    ids: &[lumina_core::MediaId],
    index: usize,
) {
    click(vcx, card(ids[index]));
    keystrokes(vcx, "alt-secondary-s");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_some(), "⌥⌘S starts the slideshow");
        assert!(app.is_fullscreen_viewer, "it plays in the viewer");
        assert!(app.is_immersive_fullscreen, "the immersive viewer");
        assert_eq!(app.selected_index, Some(index), "from the selection");
    });
}

#[core::prelude::v1::test]
fn slideshow_advances_on_clock() {
    let (_dir, engine) = seed_library("slideshow_clock", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    start_at(&view, vcx, &ids, 0);
    assert!(
        vcx.debug_bounds("slideshow-view").is_some(),
        "the slideshow paints its stage"
    );
    assert!(
        vcx.update(|window, _| window.is_fullscreen()),
        "the window goes full screen"
    );

    idle(vcx, INTERVAL - Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(0), "a slide stays its interval");
    idle(vcx, Duration::from_millis(200));
    assert_eq!(shown(&view, vcx), Some(1), "then the next slide shows");
    idle(vcx, INTERVAL);
    assert_eq!(shown(&view, vcx), Some(2), "and the next, on the clock");

    // The interval comes from the config (⌘K Slideshow Interval).
    keystrokes(vcx, "escape");
    view.update(vcx, |app, cx| {
        app.execute_command(
            crate::app::command_palette::CommandId::SlideshowInterval(3),
            cx,
        );
        assert_eq!(app.config.slideshow_interval_secs, 3);
    });
    start_at(&view, vcx, &ids, 0);
    idle(vcx, Duration::from_millis(3100));
    assert_eq!(shown(&view, vcx), Some(1), "a 3 second interval");
    keystrokes(vcx, "escape");

    // View > Slideshow (the menu action) starts it too.
    vcx.dispatch_action(crate::actions::Slideshow);
    render_ui(vcx);
    assert!(playing(&view, vcx), "View > Slideshow starts the slideshow");
    keystrokes(vcx, "escape");

    // So does ⌘K Slideshow (deferred into the app's window, which it needs
    // for full screen).
    view.update(vcx, |app, cx| {
        app.execute_command(crate::app::command_palette::CommandId::Slideshow, cx)
    });
    render_ui(vcx);
    assert!(playing(&view, vcx), "⌘K Slideshow starts the slideshow");
    keystrokes(vcx, "escape");
}

#[core::prelude::v1::test]
fn space_pauses_and_arrows_step() {
    let (_dir, engine) = seed_library("slideshow_space", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    start_at(&view, vcx, &ids, 0);

    keystrokes(vcx, "space");
    assert!(paused(&view, vcx), "Space pauses");
    assert!(
        vcx.debug_bounds("slideshow-paused").is_some(),
        "a paused slideshow says so"
    );
    idle(vcx, INTERVAL * 3);
    assert_eq!(shown(&view, vcx), Some(0), "a paused slideshow stays put");

    keystrokes(vcx, "right");
    assert_eq!(shown(&view, vcx), Some(1), "→ steps forward");
    keystrokes(vcx, "right");
    keystrokes(vcx, "left");
    assert_eq!(shown(&view, vcx), Some(1), "← steps back");
    assert!(paused(&view, vcx), "stepping keeps it paused");

    keystrokes(vcx, "space");
    assert!(!paused(&view, vcx), "Space resumes");
    assert!(vcx.debug_bounds("slideshow-paused").is_none());
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(2), "and the clock runs again");

    // A step while playing gives the new slide a full interval.
    idle(vcx, INTERVAL - Duration::from_secs(1));
    keystrokes(vcx, "left");
    assert_eq!(shown(&view, vcx), Some(1));
    idle(vcx, INTERVAL - Duration::from_millis(500));
    assert_eq!(shown(&view, vcx), Some(1), "the timer restarts on a step");
    keystrokes(vcx, "escape");
}

#[core::prelude::v1::test]
fn esc_exits_to_previous_selection() {
    let (_dir, engine) = seed_library("slideshow_esc", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    click(vcx, card(ids[1]));
    keystrokes(vcx, "shift-right");
    let before = view.update(vcx, |app, _| {
        (
            app.selected_indices.clone(),
            app.selected_index,
            app.selected_item.as_ref().map(|item| item.id),
        )
    });
    assert_eq!(before.1, Some(2));

    keystrokes(vcx, "alt-secondary-s");
    assert_eq!(shown(&view, vcx), Some(2), "plays from the focused item");
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(3));

    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "Esc ends the slideshow");
        assert!(!app.is_fullscreen_viewer, "and closes the viewer");
        assert!(!app.is_immersive_fullscreen, "and leaves immersive mode");
        assert_eq!(
            (
                app.selected_indices.clone(),
                app.selected_index,
                app.selected_item.as_ref().map(|item| item.id),
            ),
            before,
            "one Esc puts the selection back as it was before the slideshow"
        );
    });
    assert!(
        !vcx.update(|window, _| window.is_fullscreen()),
        "the window leaves full screen"
    );
    idle(vcx, INTERVAL * 2);
    assert_eq!(
        shown(&view, vcx),
        Some(2),
        "no timer moves the selection after Esc"
    );

    // With nothing selected it plays from the start and Esc clears again.
    view.update(vcx, |app, cx| app.clear_selection(cx));
    keystrokes(vcx, "alt-secondary-s");
    assert_eq!(
        shown(&view, vcx),
        Some(0),
        "no selection plays from the start"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.selected_indices.is_empty() && app.selected_item.is_none())
    });
}

#[core::prelude::v1::test]
fn slideshow_stops_at_end_without_loop() {
    let (_dir, engine) = seed_library("slideshow_end", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    view.update(vcx, |app, _| assert!(!app.config.slideshow_loop));
    start_at(&view, vcx, &ids, 1);

    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(2), "the last slide shows");
    idle(vcx, INTERVAL * 3);
    assert_eq!(shown(&view, vcx), Some(2), "it does not wrap without Loop");
    assert!(paused(&view, vcx), "the slideshow stops on the last slide");
    assert!(vcx.debug_bounds("slideshow-paused").is_some());
    view.update(vcx, |app, _| {
        assert!(app.is_fullscreen_viewer, "the last slide stays on screen")
    });

    keystrokes(vcx, "right");
    assert_eq!(shown(&view, vcx), Some(2), "→ at the end stays put");
    keystrokes(vcx, "escape");
    assert!(!playing(&view, vcx), "Esc still ends it");
}

#[core::prelude::v1::test]
fn slideshow_loop_wraps_to_first() {
    let (_dir, engine) = seed_library("slideshow_loop", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);

    // ⌘K "Slideshow: Loop" turns it on.
    keystrokes(vcx, "secondary-k");
    for ch in "slideshow: loop".chars() {
        keystrokes(
            vcx,
            if ch == ' ' {
                "space"
            } else {
                crate::tests::helpers::leak(ch.to_string())
            },
        );
    }
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(app.config.slideshow_loop, "the palette toggles Loop on")
    });

    start_at(&view, vcx, &ids, 2);
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(0), "Loop wraps to the first item");
    assert!(!paused(&view, vcx), "and keeps playing");
    idle(vcx, INTERVAL);
    assert_eq!(shown(&view, vcx), Some(1));
    keystrokes(vcx, "escape");

    view.update(vcx, |app, cx| {
        app.execute_command(crate::app::command_palette::CommandId::SlideshowLoop, cx);
        assert!(!app.config.slideshow_loop, "the command toggles it off");
    });
}

// Needs a real player backend; other platforms use the dummy player.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn space_on_video_pauses_slideshow_not_video() {
    let (dir, _) = seed_library("slideshow_video", 2);
    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clip.mp4");
    std::fs::copy(&clip, dir.join("video.mp4")).unwrap();
    let (engine, rx) =
        LuminaEngine::open_with_paths(dir.clone(), dir.join("catalog.db"), Some(dir.join("cache")))
            .unwrap();
    while let Ok(event) = rx.recv_timeout(Duration::from_secs(10)) {
        if let lumina_core::EngineEvent::ScanComplete { .. } = event {
            break;
        }
    }
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    let video = view.update(vcx, |app, _| {
        (0..ids.len())
            .find(|&i| app.get_active_slice(i..i + 1)[0].media_type == MediaType::Video)
            .expect("the library holds the clip")
    });

    start_at(&view, vcx, &ids, video);
    view.update(vcx, |app, _| {
        assert!(app.video.player.is_some(), "the video's poster is loaded");
        assert!(!app.video.is_playing, "a video slide never autoplays");
    });
    view.update(vcx, |app, cx| app.wake_immersive_hud(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-back").is_some(),
        "the HUD is awake"
    );
    for gone in ["video-transport", "video-play", "viewer-pip-handoff"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{gone}: no video control on a slide"
        );
    }
    keystrokes(vcx, "space");
    view.update(vcx, |app, _| {
        assert!(!app.video.is_playing, "Space does not play the video");
    });
    assert!(paused(&view, vcx), "Space pauses the slideshow instead");
    keystrokes(vcx, "space");
    assert!(!paused(&view, vcx), "and resumes it");
    view.update(vcx, |app, _| assert!(!app.video.is_playing));

    let before = shown(&view, vcx);
    keystrokes(vcx, if video == 0 { "right" } else { "left" });
    assert_ne!(
        shown(&view, vcx),
        before,
        "←/→ on a video step the slideshow instead of seeking"
    );
    keystrokes(vcx, "escape");
}

#[core::prelude::v1::test]
fn slideshow_reduce_motion_has_no_transition() {
    let (_dir, engine) = seed_library("slideshow_motion", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);

    crate::motion::REDUCE_MOTION.with(|on| on.set(true));
    start_at(&view, vcx, &ids, 0);
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(1));
    assert!(
        vcx.debug_bounds("slideshow-fade").is_none(),
        "no slide fade under Reduce Motion"
    );
    keystrokes(vcx, "escape");

    crate::motion::REDUCE_MOTION.with(|on| on.set(false));
    start_at(&view, vcx, &ids, 0);
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert!(
        vcx.debug_bounds("slideshow-fade").is_some(),
        "without Reduce Motion each slide fades in"
    );
    keystrokes(vcx, "escape");
    crate::motion::REDUCE_MOTION.with(|on| on.set(true));
}

#[core::prelude::v1::test]
fn slideshow_absorbs_background_shortcuts() {
    let (_dir, engine) = seed_library("slideshow_absorb", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    start_at(&view, vcx, &ids, 0);

    for keys in [
        "secondary-k",
        "secondary-/",
        "backspace",
        "secondary-backspace",
        "p",
        "3",
        "f",
        "secondary-a",
        "secondary-n",
        "enter",
        "i",
    ] {
        keystrokes(vcx, keys);
        view.update(vcx, |app, _| {
            assert!(!app.command_palette_open, "{keys}: no palette");
            assert!(!app.shortcuts_overlay_open, "{keys}: no shortcut sheet");
            assert!(app.delete_confirm_state.is_none(), "{keys}: no Delete");
            assert!(app.slideshow.is_some(), "{keys}: the slideshow runs on");
            assert!(app.is_fullscreen_viewer, "{keys}: in the viewer");
            assert_eq!(app.selected_indices.len(), 1, "{keys}: one slide");
        });
    }
    assert!(flags(&engine).is_empty(), "P is absorbed");
    assert_eq!(engine.catalog().get_rating(ids[0]).unwrap(), 0, "3 too");
    for id in &ids {
        let item = engine
            .get_item_by_id(*id)
            .expect("the slide is still in the catalog");
        assert!(!item.is_trashed, "⌫ / ⌘⌫ never trash a slide");
    }
    assert!(!paused(&view, vcx), "absorbed keys do not pause");

    // ⌘, passes through (and pauses the show under the sheet).
    keystrokes(vcx, "secondary-,");
    view.update(vcx, |app, _| {
        assert!(app.preferences_open, "⌘, opens Preferences")
    });
    assert!(paused(&view, vcx), "the slideshow waits under Preferences");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.preferences_open, "Esc closes Preferences first");
        assert!(app.slideshow.is_some(), "and leaves the slideshow");
    });
    keystrokes(vcx, "escape");
    assert!(!playing(&view, vcx));
}

#[core::prelude::v1::test]
fn slideshow_stops_cleanly_when_view_empties_or_library_switches() {
    let (_dir, engine) = seed_library("slideshow_empty", 3);
    let (other, _) = seed_library("slideshow_other", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);

    // The view empties under it (the Trash is empty).
    start_at(&view, vcx, &ids, 0);
    view.update(vcx, |app, cx| app.select_album(AlbumSelection::Trash, cx));
    render_ui(vcx);
    idle(vcx, INTERVAL + Duration::from_millis(100));
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "an empty view ends the slideshow");
        assert!(!app.is_fullscreen_viewer, "and closes the viewer");
        assert!(!app.is_immersive_fullscreen);
        assert!(app.selected_item.is_none(), "nothing from the old view");
    });

    // An empty view does not start one.
    keystrokes(vcx, "alt-secondary-s");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "nothing to play");
        assert!(!app.is_fullscreen_viewer);
        assert!(app.toast.current().is_some(), "a toast says why");
    });

    // The library switches under it.
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    start_at(&view, vcx, &ids, 0);
    view.update(vcx, |app, cx| app.open_library_inner(other.clone(), cx));
    vcx.run_until_parked();
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "switching libraries ends it");
        assert!(!app.is_fullscreen_viewer, "and closes the viewer");
        assert!(!app.is_immersive_fullscreen);
    });
    idle(vcx, INTERVAL * 2);
    view.update(vcx, |app, _| {
        assert!(app.selected_item.is_none(), "no old timer selects anything")
    });
}

/// The macOS menu bar stays reachable in full screen: its items that open a
/// dialog, act on the selection or move the view are disabled while a
/// slideshow plays.
#[core::prelude::v1::test]
fn menu_actions_are_disabled_while_the_slideshow_plays() {
    let (_dir, engine) = seed_library("slideshow_menus", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    // A reject for Delete Rejected… and a trail for Go Back.
    click(vcx, card(ids[3]));
    keystrokes(vcx, "x");
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Trash, cx);
        app.select_album(AlbumSelection::AllLibrary, cx);
        // The seed scan is done; a rescan would set this again.
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    render_ui(vcx);
    start_at(&view, vcx, &ids, 0);
    let before = view.update(vcx, |app, _| {
        (
            app.selected_indices.clone(),
            app.selected_index,
            app.sidebar_collapsed,
            app.inspector_open,
        )
    });

    type Dispatch = fn(&mut VisualTestContext);
    let actions: Vec<(&str, Dispatch)> = vec![
        ("Delete Rejected…", |v| {
            v.dispatch_action(crate::actions::DeleteRejected)
        }),
        ("Add to Album…", |v| {
            v.dispatch_action(crate::actions::AddToAlbum)
        }),
        ("Rotate Left", |v| {
            v.dispatch_action(crate::actions::RotateLeft)
        }),
        ("Rotate Right", |v| {
            v.dispatch_action(crate::actions::RotateRight)
        }),
        ("Edit Photo", |v| {
            v.dispatch_action(crate::actions::EditPhoto)
        }),
        ("Revert", |v| {
            v.dispatch_action(crate::actions::RevertToOriginal)
        }),
        ("Command Palette", |v| {
            v.dispatch_action(crate::actions::ShowCommandPalette)
        }),
        ("New Album", |v| v.dispatch_action(crate::actions::NewAlbum)),
        ("New Smart Album", |v| {
            v.dispatch_action(crate::actions::NewSmartAlbum)
        }),
        ("Keyboard Shortcuts", |v| {
            v.dispatch_action(crate::actions::ShowShortcuts)
        }),
        ("Toggle Sidebar", |v| {
            v.dispatch_action(crate::actions::ToggleSidebar)
        }),
        ("Toggle Inspector", |v| {
            v.dispatch_action(crate::actions::ToggleInspector)
        }),
        ("Preferences", |v| {
            v.dispatch_action(crate::actions::OpenPreferences)
        }),
        ("About", |v| v.dispatch_action(crate::actions::ShowAbout)),
        ("Go Back", |v| v.dispatch_action(crate::actions::GoBack)),
        ("Go Forward", |v| {
            v.dispatch_action(crate::actions::GoForward)
        }),
        ("Compare", |v| v.dispatch_action(crate::actions::Compare)),
        ("Metadata Templates", |v| {
            v.dispatch_action(crate::actions::ManageTemplates)
        }),
        ("Open Library…", |v| {
            v.dispatch_action(crate::actions::OpenLibrary)
        }),
        ("Import…", |v| {
            v.dispatch_action(crate::actions::ImportMedia)
        }),
        ("Import Video from URL…", |v| {
            v.dispatch_action(crate::actions::ImportNetworkVideo)
        }),
        ("Rescan Library", |v| {
            v.dispatch_action(crate::actions::RescanLibrary)
        }),
        ("Show Downloads", |v| {
            v.dispatch_action(crate::actions::ToggleDownloads)
        }),
        ("Show All Downloads", |v| {
            v.dispatch_action(crate::actions::ShowAllDownloads)
        }),
        ("Import XMP Metadata", |v| {
            v.dispatch_action(crate::actions::ImportXmp)
        }),
        ("Import Smart Album Rules…", |v| {
            v.dispatch_action(crate::actions::ImportSmartAlbumRules)
        }),
        ("Edit in External Editor", |v| {
            v.dispatch_action(crate::actions::EditInExternalEditor)
        }),
        ("Edit a Copy in External Editor", |v| {
            v.dispatch_action(crate::actions::EditCopyInExternalEditor)
        }),
    ];
    let picks = || {
        (
            crate::app::library::FOLDER_PICKS.with(std::cell::Cell::get),
            crate::app::import::IMPORT_PICKS.with(std::cell::Cell::get),
            crate::app::smart_album_share::RULE_FILE_DIALOGS.with(std::cell::Cell::get),
        )
    };
    let dialogs_before = picks();
    crate::app::external_edit::test_launches::take();
    for (name, action) in actions {
        action(vcx);
        render_ui(vcx);
        view.update(vcx, |app, _| {
            assert!(app.slideshow.is_some(), "{name}: the slideshow plays on");
            assert!(
                app.delete_confirm_state.is_none(),
                "{name}: no Delete dialog"
            );
            assert!(app.albums.picker.is_none(), "{name}: no album picker");
            assert!(app.edits.session.is_none(), "{name}: no edit mode");
            assert!(!app.command_palette_open, "{name}: no palette");
            assert!(!app.shortcuts_overlay_open, "{name}: no shortcut sheet");
            assert!(!app.preferences_open, "{name}: no Preferences");
            assert!(
                !app.album_draft.modal_open,
                "{name}: no smart album builder"
            );
            assert!(app.compare.is_none(), "{name}: no compare");
            assert!(app.templates.editor.is_none(), "{name}: no template editor");
            assert!(!app.tags_page_shown(), "{name}: no Tag Manager");
            assert!(!app.downloads.open, "{name}: no downloads list");
            assert!(app.import.dialog.is_none(), "{name}: no import sheet");
            assert!(
                app.network_import.stage.is_none(),
                "{name}: no Import Video from URL"
            );
            assert!(app.jobs.xmp.is_none(), "{name}: no XMP import");
            assert!(
                !app.grid_state.as_ref().is_some_and(|s| s.is_scanning),
                "{name}: no rescan"
            );
            assert!(app.error_message.is_none(), "{name}: no banner");
            assert_eq!(
                app.selection,
                AlbumSelection::AllLibrary,
                "{name}: same view"
            );
            assert_eq!(
                (
                    app.selected_indices.clone(),
                    app.selected_index,
                    app.sidebar_collapsed,
                    app.inspector_open,
                ),
                before,
                "{name}: the slide, selection and panels stay"
            );
        });
    }
    assert_eq!(
        picks(),
        dialogs_before,
        "no folder, import or rules file dialog was asked for"
    );
    assert!(
        crate::app::external_edit::test_launches::take().is_empty(),
        "no external editor was launched"
    );
    assert!(engine.list_albums_with_counts().is_empty(), "no album made");
    let rotations = engine.catalog().get_rotations(&ids).unwrap();
    assert!(
        rotations.values().all(|q| *q == 0),
        "no menu rotation reached a slide: {rotations:?}"
    );
    keystrokes(vcx, "escape");
}

/// Esc puts the selection back by id: a row that leaves the view during the
/// show shifts every later row, and the same items come back.
#[core::prelude::v1::test]
fn esc_restores_the_selection_by_id_after_rows_move() {
    let (_dir, engine) = seed_library("slideshow_by_id", 6);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    click(vcx, card(ids[3]));
    keystrokes(vcx, "shift-right");
    let chosen = view.update(vcx, |app, _| app.get_selected_ids());
    assert_eq!(chosen, vec![ids[3], ids[4]]);

    keystrokes(vcx, "alt-secondary-s");
    // Another writer (the watcher, a scan) removes a row above the selection.
    engine.set_trashed(ids[0], true).unwrap();
    // As its event would: the view re-reads.
    view.update(vcx, |app, cx| app.refresh_active_view(cx));
    vcx.run_until_parked();
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none());
        assert_eq!(
            app.get_selected_ids(),
            chosen,
            "the same items are selected, not the rows now at their old positions"
        );
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(ids[4]),
            "the focused item comes back"
        );
        assert_eq!(app.selected_index, Some(3), "at its new position");
    });
}

/// The HUD keeps only Back while a slideshow plays; Back ends the show like
/// Esc, and the controls that would step, mark, play or hand off are gone.
#[core::prelude::v1::test]
fn slideshow_hud_has_only_back_and_back_restores_the_selection() {
    let (_dir, engine) = seed_library("slideshow_hud", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    start_at(&view, vcx, &ids, 1);
    view.update(vcx, |app, cx| app.wake_immersive_hud(cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("viewer-back").is_some(),
        "the woken HUD shows Back"
    );
    for gone in [
        "viewer-prev",
        "viewer-next",
        "viewer-fav-btn",
        "viewer-rate-btn",
        "viewer-edit-btn",
        "viewer-share-btn",
        "viewer-info-btn",
        "viewer-zoom-fit",
        "video-transport",
        "viewer-pip-handoff",
    ] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "{gone} is hidden during a slideshow"
        );
    }
    let stage = crate::tests::helpers::bounds_center(vcx, "slideshow-view");
    vcx.simulate_mouse_down(stage, gpui::MouseButton::Right, gpui::Modifiers::none());
    vcx.simulate_mouse_up(stage, gpui::MouseButton::Right, gpui::Modifiers::none());
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.context_menu_state.is_none(),
            "no right-click menu (Delete, Rotate) on a slide"
        )
    });

    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(2));
    // The HUD hid while the slide was up; the pointer brings it back.
    view.update(vcx, |app, cx| app.wake_immersive_hud(cx));
    render_ui(vcx);
    click(vcx, "viewer-back");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "Back ends the slideshow");
        assert!(!app.is_fullscreen_viewer && !app.is_immersive_fullscreen);
        assert_eq!(app.selected_index, Some(1), "and restores the selection");
        assert_eq!(app.selected_item.as_ref().map(|item| item.id), Some(ids[1]));
    });
    assert!(!vcx.update(|window, _| window.is_fullscreen()));
}

#[core::prelude::v1::test]
fn failed_slideshow_setting_save_reverts_and_shows_the_banner() {
    let (_dir, engine) = seed_library("slideshow_save", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        crate::app::state::SAVE_FAILS.with(|f| f.set(true));
        app.execute_command(
            crate::app::command_palette::CommandId::SlideshowInterval(10),
            cx,
        );
        assert_eq!(
            app.config.slideshow_interval_secs, 5,
            "a failed save puts the interval back"
        );
        assert!(app.error_message.take().is_some(), "and says so");
        app.execute_command(crate::app::command_palette::CommandId::SlideshowLoop, cx);
        crate::app::state::SAVE_FAILS.with(|f| f.set(false));
        assert!(!app.config.slideshow_loop, "a failed save puts Loop back");
        assert!(app.error_message.is_some(), "and says so");
    });
}

/// More rows than the slideshow holds by id.
const LARGE: usize = crate::app::slideshow::BY_ID_LIMIT + 20;

/// Selects all of a view larger than the by-id limit, focused on row 7.
fn select_all_large(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    ids: &[lumina_core::MediaId],
) -> (std::collections::BTreeSet<usize>, Option<usize>) {
    click(vcx, card(ids[7]));
    keystrokes(vcx, "secondary-a");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), LARGE, "⌘A selects the view");
        (app.selected_indices.clone(), app.selected_index)
    })
}

fn rows_read() -> usize {
    crate::app::active_view::ROWS_READ.with(std::cell::Cell::get)
}

/// ⌥⌘S and Esc after ⌘A on a large view read a bounded number of rows on
/// the UI thread, not every selected row (twice).
#[core::prelude::v1::test]
fn large_selection_slideshow_reads_bounded_rows() {
    let (_dir, engine) = seed_library("slideshow_large_reads", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    select_all_large(&view, vcx, &ids);

    let before = rows_read();
    keystrokes(vcx, "alt-secondary-s");
    assert!(playing(&view, vcx));
    keystrokes(vcx, "escape");
    assert!(!playing(&view, vcx));
    let read = rows_read() - before;
    assert!(
        read < 64,
        "starting and ending a slideshow must not read every selected row \
         on the UI thread: {read} rows read for {LARGE} selected"
    );
}

/// A large selection comes back exactly after Esc when nothing moved.
#[core::prelude::v1::test]
fn large_selection_is_restored_after_esc() {
    let (_dir, engine) = seed_library("slideshow_large_restore", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    let (indices, index) = select_all_large(&view, vcx, &ids);
    let summary = view.update(vcx, |app, _| app.batch_summary.count);

    keystrokes(vcx, "alt-secondary-s");
    idle(vcx, INTERVAL + Duration::from_millis(100));
    assert_eq!(shown(&view, vcx), Some(8), "the show moved on");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "Esc closes the viewer");
        assert_eq!(app.selected_indices, indices, "every row is selected again");
        assert_eq!(app.selected_index, index, "the focus comes back");
        assert_eq!(
            app.selected_item.as_ref().map(|item| item.id),
            Some(ids[7]),
            "the focused item is shown, not the last slide"
        );
        assert_eq!(app.selection_anchor, Some(7), "the anchor comes back");
        assert_eq!(
            app.batch_summary.count, summary,
            "the batch summary describes the whole selection"
        );
    });
}

/// An outside write that changes the count during the show clears a large
/// selection (never a wrong item), and the viewer still closes.
#[core::prelude::v1::test]
fn large_selection_is_cleared_when_rows_move() {
    let (_dir, engine) = seed_library("slideshow_large_moved", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    select_all_large(&view, vcx, &ids);

    keystrokes(vcx, "alt-secondary-s");
    // Another writer (the watcher, a scan) removes a row above the focus.
    engine.set_trashed(ids[0], true).unwrap();
    view.update(vcx, |app, cx| app.refresh_active_view(cx));
    vcx.run_until_parked();
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_none(), "Esc ends the slideshow");
        assert!(!app.is_fullscreen_viewer, "and closes the viewer");
        assert!(!app.is_immersive_fullscreen, "and leaves immersive mode");
        assert!(
            app.selected_indices.is_empty()
                && app.selected_index.is_none()
                && app.selected_item.is_none(),
            "a selection that may have moved is cleared, not put on other rows"
        );
    });
    assert!(
        !vcx.update(|window, _| window.is_fullscreen()),
        "the window leaves full screen"
    );
}

/// Selects `rows` with the focus on `focus`, as clicks would.
fn select_rows(
    view: &Entity<LuminaApp>,
    vcx: &mut VisualTestContext,
    rows: impl IntoIterator<Item = usize>,
    focus: usize,
) {
    view.update(vcx, |app, cx| {
        app.select_index(focus, cx);
        app.selected_indices = rows.into_iter().collect();
        app.update_batch_summary();
    });
    render_ui(vcx);
}

/// An outside write that reorders rows at the same count, leaving the
/// focused row in place, still clears a large selection: its old positions
/// now hold other items.
#[core::prelude::v1::test]
fn large_selection_is_cleared_when_rows_reorder_at_same_count() {
    let (_dir, engine) = seed_library("slideshow_large_reorder", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.set_sort(
            lumina_core::SortField::Rating,
            lumina_core::SortDirection::Descending,
            cx,
        )
    });
    render_ui(vcx);
    let first = ids(&view, vcx);
    engine.set_ratings(&[(first[0], 5)]).unwrap();
    view.update(vcx, |app, cx| app.refresh_active_view(cx));
    vcx.run_until_parked();
    render_ui(vcx);
    let rows = ids(&view, vcx);
    assert_eq!(rows[0], first[0], "test premise: the 5 is the top row");
    let selected = LARGE - 10;
    select_rows(&view, vcx, 0..selected, 0);

    keystrokes(vcx, "alt-secondary-s");
    assert!(playing(&view, vcx));
    // Another writer rates an unselected row: it moves to row 1, under the
    // focus, and every row between shifts down by one. The count is the same.
    let outsider = rows[LARGE - 5];
    engine.set_ratings(&[(outsider, 4)]).unwrap();
    view.update(vcx, |app, cx| app.refresh_active_view(cx));
    vcx.run_until_parked();
    assert_eq!(
        ids(&view, vcx)[..2].to_vec(),
        vec![rows[0], outsider],
        "test premise: the rated row moved under the unmoved focus"
    );
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(!app.is_fullscreen_viewer, "Esc closes the viewer");
        assert!(
            !app.get_selected_ids().contains(&outsider),
            "a row that was not selected must never come back selected"
        );
        assert!(
            app.selected_indices.is_empty() && app.selected_item.is_none(),
            "a selection the write may have moved is cleared"
        );
    });
}

/// Two picks far apart (a small count over a large span) read a bounded
/// number of rows: the by-id re-find reads the whole span.
#[core::prelude::v1::test]
fn far_apart_picks_read_bounded_rows() {
    let (_dir, engine) = seed_library("slideshow_far_picks", LARGE);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    select_rows(&view, vcx, [0, LARGE - 1], 0);

    let before = rows_read();
    keystrokes(vcx, "alt-secondary-s");
    assert!(playing(&view, vcx));
    keystrokes(vcx, "escape");
    let read = rows_read() - before;
    assert!(
        read < 64,
        "a selection spanning the view must not read the span on the UI \
         thread: {read} rows read"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selected_indices,
            [0, LARGE - 1].into_iter().collect(),
            "and the picks come back"
        );
    });
}
