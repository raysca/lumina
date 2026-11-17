//! An open album's header band: cover
//! under a scrim, "Albums › title" breadcrumb, Slideshow and More, the
//! title and the counts line.

use super::album_cards::utc_ms;
use super::albums_overview::height;
use super::ui_automation::{
    app_with_library, click, click_point, focus_app, keystrokes, render_ui, seed_library,
};
use crate::app::LuminaApp;
use crate::views::album_page::AlbumMenuItem;
use crate::views::sidebar::AlbumSelection;
use gpui::{Entity, VisualTestContext};
use lumina_core::{LuminaEngine, MediaId, MediaType};
use std::sync::Arc;

/// Two photos (14 and 18 Jun 2025) and a clip (16 Jun) as one album.
pub(super) fn library_with_album(
    prefix: &str,
) -> (
    super::downloads::TempLibrary,
    Arc<LuminaEngine>,
    i64,
    Vec<MediaId>,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let photos = engine.get_item_slice(0..2);
    // A row only (no file), so no watcher rescan resets it.
    let mut clip = photos[0].clone();
    clip.file_name = "clip.mov".to_string();
    clip.path = dir.join(&clip.file_name);
    clip.media_type = MediaType::Video;
    clip.duration_ms = Some(78_000);
    let clip = engine.catalog().insert_or_update_batch(&[clip]).unwrap()[0].id;
    let ids = vec![photos[0].id, photos[1].id, clip];
    for (id, day) in ids.iter().zip([14, 18, 16]) {
        engine.set_timestamp(*id, utc_ms(2025, 6, day)).unwrap();
    }
    let album = engine.create_album_with("Lisbon, June", &ids).unwrap();
    (super::downloads::TempLibrary(dir), engine, album.id, ids)
}

pub(super) fn open_album(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, id: i64) {
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(id), cx)
    });
    // The view and the header stats land from background reads, then paint.
    render_ui(vcx);
    render_ui(vcx);
}

pub(super) fn painted(vcx: &mut VisualTestContext, selector: String) -> bool {
    vcx.debug_bounds(super::ui_automation::leak(selector))
        .is_some()
}

#[core::prelude::v1::test]
fn album_header_shows_title_counts_and_actions() {
    let (_dir, engine, id, _ids) = library_with_album("album_header");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);

    let got = height(vcx, "album-header");
    assert!(
        (got - 180.0).abs() <= 0.5,
        "the album header band is 180 high: {got}"
    );
    assert!(
        vcx.debug_bounds("page-header").is_none(),
        "the band replaces the page header for an open album"
    );
    assert!(
        painted(vcx, "album-header-title-text:Lisbon, June".into()),
        "the title is the album's name"
    );
    assert!(
        painted(
            vcx,
            "album-header-meta-text:3 items · 2 photos, 1 video · 14 — 18 Jun 2025".into()
        ),
        "the meta line counts items by kind and spans their capture days"
    );

    click(vcx, "album-slideshow-btn");
    view.update(vcx, |app, _| {
        assert!(app.slideshow.is_some(), "Slideshow plays the album");
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.slideshow.is_none() && !app.is_fullscreen_viewer,
            "Esc leaves the slideshow and the viewer"
        );
    });
    render_ui(vcx);

    click(vcx, "album-crumb-root");
    view.update(vcx, |app, _| {
        assert!(
            app.albums_overview_shown(),
            "the Albums crumb opens the Albums overview"
        );
    });
}

#[core::prelude::v1::test]
fn album_header_more_menu_keys_and_actions() {
    let (_dir, engine, id, _ids) = library_with_album("album_header_menu");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    let menu_at = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.albums.page.menu)
    };

    // The mouse: More opens the menu, a click outside closes it.
    click(vcx, "album-more-btn");
    assert!(
        vcx.debug_bounds("album-menu").is_some(),
        "More opens the album menu"
    );
    for row in AlbumMenuItem::all() {
        assert!(
            vcx.debug_bounds(row.selector()).is_some(),
            "the menu offers {row:?}"
        );
    }
    click_point(vcx, 700.0, 600.0);
    assert_eq!(menu_at(&view, vcx), None, "a click outside closes the menu");

    // Keys: ↓ moves to Pin, ↵ pins and closes the menu.
    click(vcx, "album-more-btn");
    keystrokes(vcx, "down");
    assert_eq!(menu_at(&view, vcx), Some(1), "↓ highlights the next row");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.page.menu, None,
            "↵ runs the row and closes the menu"
        );
        assert_eq!(
            app.open_album_pinned(),
            Some(true),
            "Pin Album pins the album"
        );
    });

    // Other keys stay in the menu: with every member selected, ⌫ (Remove
    // from Album here) and ⌘⌫ (Move to Trash) change nothing. Esc closes it.
    keystrokes(vcx, "cmd-a");
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_indices.len(), 3, "⌘A selects the members");
    });
    click(vcx, "album-more-btn");
    keystrokes(vcx, "backspace");
    keystrokes(vcx, "cmd-backspace");
    assert_eq!(
        engine.query_album_count(id),
        3,
        "keys under the menu never remove the selection from the album"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_none(),
            "⌘⌫ under the menu never asks to trash the selection"
        );
    });
    assert_eq!(
        menu_at(&view, vcx),
        Some(0),
        "the menu stays open on other keys"
    );
    keystrokes(vcx, "escape");
    assert_eq!(menu_at(&view, vcx), None, "Esc closes the menu");
    assert!(vcx.debug_bounds("album-menu").is_none());

    // Delete Album… asks first; Rename opens the name field.
    let delete = AlbumMenuItem::all()
        .iter()
        .position(|row| *row == AlbumMenuItem::Delete)
        .unwrap();
    click(vcx, "album-more-btn");
    for _ in 0..delete {
        keystrokes(vcx, "down");
    }
    keystrokes(vcx, "enter");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.albums.confirm_delete,
            Some(id),
            "Delete Album… opens the delete confirmation"
        );
        app.cancel_delete_album(cx);
    });
    render_ui(vcx);
    click(vcx, "album-more-btn");
    click(vcx, "album-menu-rename");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.renaming, Some(id), "Rename opens the name field");
    });
}

/// During a scan the header rereads its stats only when the album's
/// member count moved: a member removed mid-scan shows at once.
#[core::prelude::v1::test]
fn album_header_stats_follow_membership_during_a_scan() {
    let (_dir, engine, id, ids) = library_with_album("album_header_scan");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_album(&view, vcx, id);
    view.update(vcx, |app, _| {
        app.grid_state.as_mut().unwrap().is_scanning = true;
    });
    engine.remove_from_album(id, &[ids[2]]).unwrap();
    render_ui(vcx);
    render_ui(vcx);
    assert!(
        painted(
            vcx,
            "album-header-meta-text:2 items · 2 photos, 0 videos · 14 — 18 Jun 2025".into()
        ),
        "the open album's stats follow a membership change mid-scan"
    );
}

/// Without a selection Share… is disabled, and ↓ from Pin skips it.
#[cfg(target_os = "macos")]
#[core::prelude::v1::test]
fn album_menu_keys_skip_disabled_share() {
    let (_dir, engine, id, _ids) = library_with_album("album_menu_skip");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    focus_app(vcx, &view);
    view.update(vcx, |app, cx| app.clear_selection(cx));
    click(vcx, "album-more-btn");
    let index = |row| AlbumMenuItem::all().iter().position(|r| *r == row).unwrap();
    keystrokes(vcx, "down");
    keystrokes(vcx, "down");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.page.menu,
            Some(index(AlbumMenuItem::Delete)),
            "↓ skips the disabled Share… row"
        );
    });
    keystrokes(vcx, "up");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.page.menu,
            Some(index(AlbumMenuItem::Pin)),
            "↑ skips it too"
        );
    });
}

/// Sort and Add filter hang from the band's own buttons: the band stays
/// put (no page header swaps in) and the popover paints.
#[core::prelude::v1::test]
fn album_header_popovers_keep_the_band() {
    let (_dir, engine, id, _ids) = library_with_album("album_header_popovers");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, id);
    let band = vcx.debug_bounds("album-header").expect("album-header");
    for selector in ["inspector-toggle", "sort-menu-button", "header-filter"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "the band keeps the page header's {selector}"
        );
    }

    click(vcx, "sort-menu-button");
    assert!(
        vcx.debug_bounds("sort-menu").is_some(),
        "Sort opens its menu"
    );
    assert_eq!(
        vcx.debug_bounds("album-header"),
        Some(band),
        "the band stays put under the Sort menu"
    );
    assert!(
        vcx.debug_bounds("page-header").is_none(),
        "no page header swaps in"
    );
    keystrokes(vcx, "escape");

    // ⌘K Add Filter.
    view.update(vcx, |app, cx| {
        app.execute_command(crate::app::command_palette::CommandId::AddFilter, cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("filter-add-menu").is_some(),
        "the palette's Add filter opens the menu under the band's Filter"
    );
    assert_eq!(
        vcx.debug_bounds("album-header"),
        Some(band),
        "the band stays put under the Add filter menu"
    );
}

#[core::prelude::v1::test]
fn album_header_empty_album_shows_zero_items() {
    let (dir, engine) = seed_library("album_header_empty", 1);
    let _dir = super::downloads::TempLibrary(dir);
    let album = engine.create_album_with("Nothing yet", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_album(&view, vcx, album.id);
    assert!(
        painted(vcx, "album-header-title-text:Nothing yet".into()),
        "an empty album keeps its header and title"
    );
    assert!(
        painted(vcx, "album-header-meta-text:0 items".into()),
        "an empty album reads 0 items, with no kinds or date range"
    );
}
