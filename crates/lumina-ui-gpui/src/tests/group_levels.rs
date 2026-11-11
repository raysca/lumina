//! Years / Months / Days on the zoom pill (plan 01b Task 7): the segments
//! show only in Mosaic where the timeline rail is, the cards come from the
//! cached timeline buckets, and a card opens Days at its period.

use gpui::{Entity, VisualTestContext};

use crate::app::command_palette::CommandId;
use crate::config::{BrowseLayout, GroupLevel};
use crate::tests::mosaic_geometry::{layout, rail_library, tall_mosaic, top_ix};
use crate::tests::ui_automation::{click, focus_app, keystrokes, render_ui};
use crate::views::mosaic::RowKind;
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;

fn level(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> GroupLevel {
    view.update(vcx, |app, _| app.config.group_level)
}

fn shown(vcx: &mut VisualTestContext, selector: &'static str) -> bool {
    vcx.debug_bounds(selector).is_some()
}

/// The top row is the day header of item `first`.
fn assert_header_on_top(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, first: usize) {
    let layout = layout(view, vcx);
    let row = layout.rows[top_ix(view, vcx)];
    assert!(
        matches!(row.kind, RowKind::Header { .. }) && row.first == first,
        "the period's day header is on top: {row:?}"
    );
}

#[core::prelude::v1::test]
fn zoom_pill_levels_show_year_and_month_cards() {
    let (dir, engine) = rail_library("levels_cards");
    let cover = engine
        .get_timeline_buckets("year")
        .into_iter()
        .find(|b| b.bucket_key == "2025")
        .and_then(|b| b.cover_media_id)
        .expect("2025 has a cover");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    assert_eq!(level(&view, vcx), GroupLevel::Days, "Days by default");
    for selector in ["zoom-level-years", "zoom-level-months", "zoom-level-days"] {
        assert!(shown(vcx, selector), "{selector} renders in Mosaic");
    }
    assert!(!shown(vcx, "zoom-level-moments"), "Moments is dropped");

    click(vcx, "zoom-level-years");
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Years);
    assert!(shown(vcx, "year-card-2025") && shown(vcx, "year-card-2024"));
    assert!(!shown(vcx, "mosaic-list"), "the cards replace the day rows");
    view.update(vcx, |app, _| {
        let cards = app.bucket_cards();
        assert_eq!(
            cards[0].aria_label, "2025, 28 items",
            "from the year bucket"
        );
        assert_eq!(cards[1].aria_label, "2024, 32 items");
        assert_eq!(cards[0].cover, Some(cover));
        let state = app.grid_state.as_ref().unwrap();
        assert!(
            state.in_flight.lock().contains(&cover)
                || state.is_failed(cover)
                || state.texture_cache.lock().contains(&cover),
            "the cover's thumbnail was requested"
        );
    });

    // ↵ on a month card (arrows move the focus) opens Days at that month.
    click(vcx, "zoom-level-months");
    render_ui(vcx);
    assert!(shown(vcx, "month-card-202506") && shown(vcx, "month-card-202403"));
    focus_app(vcx, &view);
    keystrokes(vcx, "right right");
    view.update(vcx, |app, _| {
        assert_eq!(app.bucket_cards_focus(), Some(2), "arrows move the focus")
    });
    keystrokes(vcx, "left right");
    keystrokes(vcx, "enter");
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Days);
    assert!(shown(vcx, "mosaic-list"));
    // 2024-12 starts after 20 + 8 items.
    assert_header_on_top(&view, vcx, 28);
    view.update(vcx, |app, _| {
        assert_eq!(app.selected_index, None, "↵ on a card selects nothing")
    });

    // A year card click does the same.
    click(vcx, "zoom-level-years");
    render_ui(vcx);
    click(vcx, "year-card-2025");
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Days);
    assert_header_on_top(&view, vcx, 0);

    // A rail click under Years opens Days there too.
    click(vcx, "zoom-level-years");
    render_ui(vcx);
    click(vcx, "timeline-year-2024");
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Days, "the rail opens Days");
    assert_header_on_top(&view, vcx, 28);

    // Esc from Months goes back to Days.
    click(vcx, "zoom-level-months");
    render_ui(vcx);
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Days, "Esc returns to Days");

    // In Grid the segments are absent.
    view.update(vcx, |app, cx| app.set_browse_layout(BrowseLayout::Grid, cx));
    render_ui(vcx);
    assert!(!shown(vcx, "zoom-level-days"), "no segments in Grid");
    assert!(shown(vcx, "grid-zoom-in"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn levels_are_absent_off_the_rail_and_the_body_is_days() {
    let (dir, engine) = rail_library("levels_album");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    click(vcx, "zoom-level-years");
    render_ui(vcx);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Untagged, cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    assert!(!shown(vcx, "zoom-level-years"), "no segments off the rail");
    assert!(!shown(vcx, "year-card-2025"), "and no cards");
    assert!(shown(vcx, "mosaic-list"), "the body is Days");
    view.update(vcx, |app, _| {
        assert!(!app.command_applies(CommandId::ShowMonths));
        assert!(!app.command_applies(CommandId::ShowDays));
    });
    let columns = view.update(vcx, |app, _| app.grid_columns);
    click(vcx, "grid-zoom-in");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_ne!(app.grid_columns, columns, "± still works")
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn palette_shows_levels_only_where_the_segments_render() {
    let (dir, engine) = rail_library("levels_palette");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    view.update(vcx, |app, cx| {
        assert!(app.command_applies(CommandId::ShowYears));
        assert!(app.command_applies(CommandId::ShowMonths));
        assert!(!app.command_applies(CommandId::ShowDays), "already on Days");
        app.execute_command(CommandId::ShowYears, cx);
        assert_eq!(app.config.group_level, GroupLevel::Years);
        assert!(app.command_applies(CommandId::ShowDays));
        app.execute_command(CommandId::ShowDays, cx);
        app.set_browse_layout(BrowseLayout::Grid, cx);
        assert!(!app.command_applies(CommandId::ShowYears), "not in Grid");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_failed_save_keeps_the_level() {
    let (dir, engine) = rail_library("levels_save_fails");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    crate::app::state::SAVE_FAILS.with(|f| f.set(true));
    click(vcx, "zoom-level-years");
    crate::app::state::SAVE_FAILS.with(|f| f.set(false));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.config.group_level, GroupLevel::Days, "reverted");
        assert!(app.error_message.is_some(), "the failure is in the banner");
    });
    assert!(shown(vcx, "mosaic-list"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn a_card_opened_before_the_first_day_layout_lands_on_its_period() {
    let (dir, engine) = rail_library("levels_card_early");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    click(vcx, "zoom-level-years");
    render_ui(vcx);
    let offset = view.update(vcx, |app, cx| {
        // A new view (oldest first): no day layout for it yet.
        app.sort.direction = lumina_core::SortDirection::Ascending;
        assert!(app.mosaic_layout().is_none());
        let card = app
            .bucket_cards()
            .into_iter()
            .find(|c| c.selector == "year-card-2025")
            .expect("a 2025 card");
        app.open_bucket_card(card.offset, cx);
        assert!(app.mosaic.pending_scroll.is_some(), "the scroll is queued");
        card.offset
    });
    assert_eq!(offset, 32, "2025 follows the 32 items of 2024 oldest first");
    render_ui(vcx);
    render_ui(vcx);
    assert_eq!(level(&view, vcx), GroupLevel::Days);
    assert_header_on_top(&view, vcx, offset);
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn grid_keys_change_nothing_under_years() {
    let (dir, engine) = rail_library("levels_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = tall_mosaic(&mut cx, engine);
    view.update(vcx, |app, cx| app.select_index(5, cx));
    render_ui(vcx);
    click(vcx, "zoom-level-years");
    render_ui(vcx);
    let (id, count) = view.update(vcx, |app, _| {
        (
            app.selected_item.as_ref().unwrap().id,
            app.get_active_count(),
        )
    });
    focus_app(vcx, &view);
    keystrokes(vcx, "backspace f 3");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(app.config.group_level, GroupLevel::Years);
        assert_eq!(app.selected_index, Some(5), "the selection stays");
        assert_eq!(app.selected_indices.len(), 1);
        assert!(
            app.delete_confirm_state.is_none(),
            "⌫ opens no Delete dialog"
        );
        assert_eq!(app.get_active_count(), count, "nothing went to the trash");
        let item = app.displayed_rows(5..6)[0].clone();
        assert_eq!(item.id, id);
        assert!(!item.is_favorite, "F does not favourite");
        assert_eq!(app.get_item_rating(&item), 0, "3 does not rate");
    });
    let _ = std::fs::remove_dir_all(&dir);
}
