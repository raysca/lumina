//! Design 05 Task 5: smart album cards on the Albums overview (frame
//! `a0sLrG`): the Albums tab's row of up to four, the Smart Albums tab's
//! wrapping grid, their mosaics, counts, rule chips, focus and menus.

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::album_cards::open_overview_at_1600;
use super::albums_overview::height;
use super::ui_automation::{
    app_with_library, bounds_center, click, focus_app, keystrokes, leak, render_ui, right_click,
    seed_library,
};
use crate::app::albums_overview::OverviewFocus;
use crate::views::sidebar::AlbumSelection;

pub(super) fn iso_over_1600() -> SmartAlbumRule {
    SmartAlbumRule {
        field: RuleField::Iso,
        operator: RuleOperator::GreaterThan,
        value: RuleValue::Integer(1600),
    }
}

pub(super) fn photos() -> SmartAlbumRule {
    SmartAlbumRule {
        field: RuleField::MediaType,
        operator: RuleOperator::Equals,
        value: RuleValue::Text("photo".into()),
    }
}

fn painted(vcx: &mut gpui::VisualTestContext, selector: String) -> bool {
    vcx.debug_bounds(leak(selector)).is_some()
}

#[core::prelude::v1::test]
fn smart_album_cards_show_mosaic_count_and_chips() {
    let (dir, engine) = seed_library("smart_cards_card", 5);
    // ISO > 1600 or a photo: every seeded JPEG matches.
    let night = engine
        .create_smart_album(
            "Night",
            "sparkles",
            MatchMode::Any,
            &[iso_over_1600(), photos()],
        )
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    render_ui(vcx);

    let card = leak(format!("smart-card-{}", night.id));
    let got = height(vcx, card);
    assert!(
        (got - 138.0).abs() <= 0.5,
        "a smart card is 138 high: {got}"
    );
    for n in 0..4 {
        let tile = format!("smart-card-tile-{}-{n}", night.id);
        assert!(painted(vcx, tile.clone()), "the mosaic paints {tile}");
        let got = height(vcx, leak(tile));
        assert!((got - 55.0).abs() <= 0.5, "a mosaic tile is 55 high: {got}");
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.smart_tiles.get(&night.id).map(Vec::len),
            Some(4),
            "the mosaic holds the album's first 4 matches, read off the UI thread"
        );
    });
    assert!(
        painted(vcx, "smart-card-meta-text:5 items · live".into()),
        "the count reads \"N items · live\""
    );
    assert!(
        painted(vcx, "smart-chip-text:ISO > 1600".into()),
        "a chip per rule: ISO > 1600"
    );
    assert!(
        painted(vcx, format!("smart-chip-{}-1", night.id)),
        "the second rule has its own chip"
    );

    click(vcx, card);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::User(night.id),
            "a click opens the smart album"
        );
    });
    keystrokes(vcx, "cmd-[");
    render_ui(vcx);

    // Keyboard: the tab bar, New Album, then the first smart card.
    focus_app(vcx, &view);
    for _ in 0..3 {
        keystrokes(vcx, "tab");
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus,
            Some(OverviewFocus::SmartCard(night.id)),
            "the third Tab focuses the first smart card"
        );
    });
    super::duplicates::run_palette_command(vcx, &view, "Edit Smart Album");
    view.update(vcx, |app, cx| {
        assert!(app.album_draft.modal_open, "⌘K Edit Smart Album… opens");
        assert_eq!(
            app.album_draft.editing_album_id,
            Some(night.id),
            "and edits the focused card"
        );
        app.close_album_builder(cx);
    });
    super::duplicates::run_palette_command(vcx, &view, "Delete Smart Album");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.album_draft.confirm_delete,
            Some(night.id),
            "⌘K Delete Smart Album… asks about the focused card"
        );
        app.cancel_delete_smart_album(cx);
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::User(night.id),
            "↵ opens the focused smart card"
        );
    });
    keystrokes(vcx, "cmd-[");
    render_ui(vcx);

    click(vcx, "manage-rules");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.tab,
            crate::app::albums_overview::AlbumsTab::SmartAlbums,
            "Manage rules switches to the Smart Albums tab"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn smart_tab_without_smart_albums_offers_new_smart_album() {
    let (dir, engine) = seed_library("smart_cards_empty", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    assert!(
        vcx.debug_bounds("smart-empty").is_some(),
        "the Albums tab's smart section says there are none"
    );
    click(vcx, "page-tab-smart-albums");
    assert!(
        vcx.debug_bounds("smart-empty").is_some(),
        "the Smart Albums tab shows a one-line empty state"
    );
    click(vcx, "smart-empty-new");
    view.update(vcx, |app, _| {
        assert!(
            app.album_draft.modal_open && app.album_draft.editing_album_id.is_none(),
            "its New Smart Album opens the builder (as ⌥⌘N)"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn albums_tab_shows_four_smart_cards_and_the_smart_tab_all() {
    let (dir, engine) = seed_library("smart_cards_many", 1);
    let ids: Vec<i64> = (0..6)
        .map(|i| {
            engine
                .create_smart_album(&format!("S{i}"), "sparkles", MatchMode::All, &[photos()])
                .unwrap()
                .id
        })
        .collect();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    let card = |id: i64| format!("smart-card-{id}");
    let shown = |vcx: &mut gpui::VisualTestContext| {
        ids.iter().filter(|id| painted(vcx, card(**id))).count()
    };
    assert_eq!(shown(vcx), 4, "the Albums tab shows four smart cards");
    let y = |vcx: &mut gpui::VisualTestContext, id: i64| {
        f32::from(bounds_center(vcx, leak(card(id))).y)
    };
    assert_eq!(y(vcx, ids[0]), y(vcx, ids[3]), "in one row");

    click(vcx, "page-tab-smart-albums");
    assert_eq!(shown(vcx), 6, "the Smart Albums tab shows every one");
    assert_eq!(y(vcx, ids[0]), y(vcx, ids[3]), "four per row");
    assert!(y(vcx, ids[4]) > y(vcx, ids[0]), "the rest wrap");

    vcx.simulate_resize(gpui::size(gpui::px(860.0), gpui::px(1000.0)));
    render_ui(vcx);
    assert_eq!(y(vcx, ids[0]), y(vcx, ids[1]), "two per row when narrow");
    assert!(y(vcx, ids[2]) > y(vcx, ids[1]), "the third wraps");
    let right = vcx
        .debug_bounds(leak(card(ids[1])))
        .expect("painted")
        .right();
    assert!(
        f32::from(right) <= 860.0,
        "no card runs off the window: {right:?}"
    );
    view.update(vcx, |app, _| assert!(app.albums_overview_shown()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn switching_tabs_drops_the_smart_focus_and_menu() {
    let (dir, engine) = seed_library("smart_cards_tabs", 1);
    let a = engine
        .create_smart_album("A", "sparkles", MatchMode::All, &[photos()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    focus_app(vcx, &view);
    for _ in 0..3 {
        keystrokes(vcx, "tab");
    }
    assert_eq!(
        view.update(vcx, |app, _| app.albums.overview.focus),
        Some(OverviewFocus::SmartCard(a.id))
    );
    click(vcx, "page-tab-smart-albums");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.albums.overview.focus, None,
            "the smart focus is dropped"
        );
    });

    // The right-click menu offers Edit and Delete, and a tab change closes it.
    let card = leak(format!("smart-card-{}", a.id));
    right_click(vcx, card);
    click(vcx, "smart-card-menu-edit");
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.album_draft.editing_album_id,
            Some(a.id),
            "Edit opens the builder on the album"
        );
        app.close_album_builder(cx);
    });
    render_ui(vcx);
    right_click(vcx, card);
    click(vcx, "smart-card-menu-delete");
    view.update(vcx, |app, cx| {
        assert_eq!(app.album_draft.confirm_delete, Some(a.id), "Delete asks");
        app.cancel_delete_smart_album(cx);
    });
    render_ui(vcx);
    right_click(vcx, card);
    super::duplicates::run_palette_command(vcx, &view, "Show Albums Tab");
    view.update(vcx, |app, _| {
        assert!(app.albums.overview.card_menu.is_none(), "the menu closed");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn smart_row_and_album_grid_scroll_as_one_page() {
    let (dir, engine) = seed_library("smart_cards_scroll", 1);
    let smart = engine
        .create_smart_album("A", "sparkles", MatchMode::All, &[photos()])
        .unwrap();
    for i in 0..59 {
        engine.create_album(&format!("Album {i:02}")).unwrap();
    }
    let last = engine.create_album("Album 59").unwrap().id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_overview_at_1600(vcx);
    let smart_card = format!("smart-card-{}", smart.id);
    assert!(painted(vcx, smart_card.clone()));
    assert!(!painted(vcx, format!("album-card-{last}")));
    // Tab to the first album card (tabs, New Album, smart card, Sort,
    // card), then ↓ to the last row.
    focus_app(vcx, &view);
    for _ in 0..5 {
        keystrokes(vcx, "tab");
    }
    assert!(matches!(
        view.update(vcx, |app, _| app.albums.overview.focus),
        Some(OverviewFocus::Card(_))
    ));
    for _ in 0..12 {
        keystrokes(vcx, "down");
    }
    render_ui(vcx);
    assert!(
        painted(vcx, format!("album-card-{last}")),
        "the last row scrolls into view"
    );
    assert!(
        !painted(vcx, smart_card.clone()) && vcx.debug_bounds("manage-rules").is_none(),
        "the smart row scrolled away with the page"
    );
    // Shift-Tab back to the smart card scrolls the top back in.
    keystrokes(vcx, "shift-tab");
    keystrokes(vcx, "shift-tab");
    render_ui(vcx);
    assert!(
        painted(vcx, smart_card),
        "the focused smart card scrolls in"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
