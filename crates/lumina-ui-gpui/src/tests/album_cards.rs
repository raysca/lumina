//! Design 05 Task 4: the album card grid on the Albums overview's Albums
//! tab (frame `a0sLrG`).

use super::albums_overview::height;
use super::ui_automation::{
    app_with_library, bounds_center, click, focus_app, keystrokes, leak, render_ui, right_click,
    seed_library,
};
use crate::app::albums_overview::OverviewFocus;
use crate::views::sidebar::AlbumSelection;

/// Epoch ms of a UTC date, for capture times.
pub(super) fn utc_ms(year: i32, month: u32, day: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_opt(12, 0, 0))
        .expect("a valid test date")
        .and_utc()
        .timestamp_millis()
}

/// The overview at a 1600 × 1000 window, with the album covers loaded.
pub(super) fn open_overview_at_1600(vcx: &mut gpui::VisualTestContext) {
    vcx.simulate_resize(gpui::size(gpui::px(1600.0), gpui::px(1000.0)));
    click(vcx, "sidebar-albums-header");
    // The covers land from the background read, then paint.
    render_ui(vcx);
}

fn width(vcx: &mut gpui::VisualTestContext, selector: &'static str) -> f32 {
    let bounds = vcx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is painted"));
    f32::from(bounds.size.width)
}

#[core::prelude::v1::test]
fn album_cards_render_and_open() {
    let (dir, engine) = seed_library("album_cards_render", 3);
    let items = engine.get_item_slice(0..3);
    engine
        .set_timestamp(items[0].id, utc_ms(2025, 6, 14))
        .unwrap();
    engine
        .set_timestamp(items[1].id, utc_ms(2024, 1, 2))
        .unwrap();
    engine
        .set_timestamp(items[2].id, utc_ms(2023, 3, 4))
        .unwrap();
    let lisbon = engine.create_album("Lisbon").unwrap();
    engine
        .add_to_album(lisbon.id, &[items[0].id, items[1].id])
        .unwrap();
    let family = engine.create_album("Family").unwrap();
    engine.add_to_album(family.id, &[items[2].id]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_overview_at_1600(vcx);

    let card = leak(format!("album-card-{}", lisbon.id));
    let cover = leak(format!("album-card-cover-{}", lisbon.id));
    let got = width(vcx, card);
    assert!(
        (got - 245.0).abs() <= 0.5,
        "an album card is 245 wide at a 1600 window: {got}"
    );
    let got = height(vcx, cover);
    assert!((got - 183.0).abs() <= 0.5, "the cover is 183 high: {got}");
    for marker in [
        "album-card-title-text:Lisbon".to_string(),
        "album-card-meta-text:2 items · Jun 2025".to_string(),
        "album-card-meta-text:1 item · Mar 2023".to_string(),
    ] {
        assert!(
            vcx.debug_bounds(leak(marker.clone())).is_some(),
            "the caption paints {marker}"
        );
    }

    right_click(vcx, card);
    assert!(
        vcx.debug_bounds("album-card-menu-rename").is_some()
            && vcx.debug_bounds("album-card-menu-delete").is_some(),
        "right-click offers Rename and Delete"
    );
    click(vcx, "album-card-menu-rename");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.albums.renaming,
            Some(lisbon.id),
            "Rename opens the album's name field"
        );
        app.cancel_album_rename(cx);
    });
    render_ui(vcx);
    right_click(vcx, card);
    click(vcx, "album-card-menu-delete");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.albums.confirm_delete,
            Some(lisbon.id),
            "Delete asks before deleting the album"
        );
        app.cancel_delete_album(cx);
    });
    render_ui(vcx);

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Album(lisbon.id),
            "a click opens the album"
        );
    });

    // Keyboard: Tab past the tabs, New Album and Sort reaches the first
    // card (by name, Family); → moves to Lisbon, ↵ opens it.
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-[");
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus,
            Some(OverviewFocus::Card(family.id)),
            "the fourth Tab focuses the first card"
        );
    });
    keystrokes(vcx, "right");
    keystrokes(vcx, "cmd-backspace");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.albums.confirm_delete,
            Some(lisbon.id),
            "⌘⌫ asks to delete the focused album"
        );
        app.cancel_delete_album(cx);
    });
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Album(lisbon.id),
            "↵ opens the focused album"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_sort_is_saved_and_reverts_on_a_failed_save() {
    use crate::config::AlbumSort;
    let (dir, engine) = seed_library("album_cards_sort", 3);
    let items = engine.get_item_slice(0..3);
    let small = engine.create_album("Alpha").unwrap();
    engine.add_to_album(small.id, &[items[0].id]).unwrap();
    let big = engine.create_album("Beta").unwrap();
    engine
        .add_to_album(big.id, &[items[1].id, items[2].id])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    let x = |vcx: &mut gpui::VisualTestContext, id: i64| {
        f32::from(bounds_center(vcx, leak(format!("album-card-{id}"))).x)
    };
    assert!(x(vcx, small.id) < x(vcx, big.id), "Name sorts Alpha first");

    click(vcx, "albums-sort");
    click(vcx, "albums-sort-count");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.album_sort,
            AlbumSort::ItemCount,
            "saved in the config"
        );
        assert!(app.error_message.is_none());
    });
    assert!(
        vcx.debug_bounds("albums-sort-menu").is_none(),
        "picking a field closes the menu"
    );
    assert!(
        x(vcx, big.id) < x(vcx, small.id),
        "Item count puts Beta first"
    );

    // Keyboard: Tab to Sort, ↵ opens the menu, ↑ to Newest photo, ↵ picks.
    focus_app(vcx, &view);
    for _ in 0..3 {
        keystrokes(vcx, "tab");
    }
    keystrokes(vcx, "enter");
    assert!(
        vcx.debug_bounds("albums-sort-menu").is_some(),
        "↵ opens the menu"
    );
    keystrokes(vcx, "up");
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.album_sort,
            AlbumSort::NewestPhoto,
            "↑ ↵ picks a field"
        );
    });

    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "albums-sort");
    click(vcx, "albums-sort-name");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    view.update(vcx, |app, _| {
        assert_eq!(
            app.config.album_sort,
            AlbumSort::NewestPhoto,
            "a failed save puts the sort back"
        );
        assert!(app.error_message.is_some(), "and says so in the banner");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn albums_tab_without_albums_offers_new_album() {
    let (dir, engine) = seed_library("album_cards_empty", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open_overview_at_1600(vcx);
    assert!(
        vcx.debug_bounds("albums-empty").is_some(),
        "a one-line empty state"
    );
    assert!(
        vcx.debug_bounds("albums-sort").is_none(),
        "no sort control over nothing"
    );
    click(vcx, "albums-empty-new");
    let albums = engine.list_albums_with_counts();
    assert_eq!(albums.len(), 1, "the empty state's New Album creates one");
    view.update(vcx, |app, _| {
        assert_eq!(app.albums.renaming, Some(albums[0].0.id), "in rename mode");
    });
    render_ui(vcx);
    assert!(vcx.debug_bounds("albums-empty").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn empty_album_card_shows_a_folder_placeholder() {
    let (dir, engine) = seed_library("album_cards_placeholder", 1);
    let item = engine.get_item_slice(0..1)[0].clone();
    let empty = engine.create_album("Empty").unwrap();
    let full = engine.create_album("Full").unwrap();
    engine.add_to_album(full.id, &[item.id]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (_view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    assert!(
        vcx.debug_bounds(leak(format!("album-card-placeholder-{}", empty.id)))
            .is_some(),
        "an empty album shows the folder placeholder"
    );
    assert!(
        vcx.debug_bounds(leak(format!("album-card-placeholder-{}", full.id)))
            .is_none(),
        "an album with a cover shows no placeholder"
    );
    assert!(
        vcx.debug_bounds("album-card-meta-text:0 items").is_some(),
        "an empty album's meta has no date"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn album_grid_is_virtualised_and_keeps_the_focus_in_view() {
    let (dir, engine) = seed_library("album_cards_many", 1);
    // 103 = 20 rows of 5 and a short last row of 3.
    let ids: Vec<i64> = (0..103)
        .map(|i| engine.create_album(&format!("Album {i:03}")).unwrap().id)
        .collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    let card = |id: i64| leak(format!("album-card-{id}"));
    assert!(vcx.debug_bounds(card(ids[0])).is_some());
    assert!(
        vcx.debug_bounds(card(ids[102])).is_none(),
        "rows below the fold are not built"
    );
    let focused = |view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, _| app.albums.overview.focus)
    };
    focus_app(vcx, &view);
    for _ in 0..4 {
        keystrokes(vcx, "tab");
    }
    keystrokes(vcx, "right");
    for _ in 0..25 {
        keystrokes(vcx, "down");
    }
    assert_eq!(
        focused(&view, vcx),
        Some(OverviewFocus::Card(ids[101])),
        "↓ reaches the short last row in the same column and stops there"
    );
    assert!(
        vcx.debug_bounds(card(ids[101])).is_some(),
        "the focused card scrolls into view"
    );
    keystrokes(vcx, "right");
    keystrokes(vcx, "right");
    assert_eq!(
        focused(&view, vcx),
        Some(OverviewFocus::Card(ids[102])),
        "→ stops at the last card"
    );
    keystrokes(vcx, "up");
    keystrokes(vcx, "right");
    keystrokes(vcx, "right");
    keystrokes(vcx, "down");
    assert_eq!(
        focused(&view, vcx),
        Some(OverviewFocus::Card(ids[99])),
        "↓ from a column the short row lacks stays put"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two albums (Alpha with an item, Beta empty) on the overview at 1600.
pub(super) fn two_album_overview<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
    [i64; 2],
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 2);
    let items = engine.get_item_slice(0..2);
    let alpha = engine.create_album("Alpha").unwrap();
    engine.add_to_album(alpha.id, &[items[0].id]).unwrap();
    let beta = engine.create_album("Beta").unwrap();
    let (view, vcx) = app_with_library(cx, engine.clone());
    open_overview_at_1600(vcx);
    (dir, engine, [alpha.id, beta.id], view, vcx)
}

#[core::prelude::v1::test]
fn a_membership_change_updates_the_card() {
    let mut cx = crate::tests::TestApp::single();
    let (dir, engine, [_, beta], view, vcx) = two_album_overview("album_cards_live", &mut cx);
    let placeholder = leak(format!("album-card-placeholder-{beta}"));
    assert!(vcx.debug_bounds(placeholder).is_some());
    let item = engine.get_item_slice(1..2)[0].clone();
    engine.set_timestamp(item.id, utc_ms(2025, 6, 14)).unwrap();
    engine.add_to_album(beta, &[item.id]).unwrap();
    view.update(vcx, |app, _| app.refresh_album_counts());
    render_ui(vcx);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds(placeholder).is_none(),
        "the album's new member becomes its cover"
    );
    assert!(
        vcx.debug_bounds("album-card-meta-text:1 item · Jun 2025")
            .is_some(),
        "the meta follows the new member"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
