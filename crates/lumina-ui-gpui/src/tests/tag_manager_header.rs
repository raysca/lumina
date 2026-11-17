//! Tags page header: meta, the Keywords tab, the filter
//! field and the Most used / A–Z sort menu.

use super::tag_manager::open;
use crate::tests::ui_automation::{app_with_library, click, keystrokes, render_ui, seed_library};
use crate::{LuminaApp, TagSortOrder};
use gpui::{Entity, VisualTestContext};

/// zebra on 2 of 3 items, apple on 1 (the same as zebra's first): 2 of 3
/// tagged, and A–Z and Most used order them differently.
fn two_keywords(
    prefix: &str,
) -> (
    std::path::PathBuf,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let (dir, engine) = seed_library(prefix, 3);
    let ids: Vec<_> = engine.get_item_slice(0..3).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids[..2], &["zebra".into()]).unwrap();
    engine.add_tags_batch(&ids[..1], &["apple".into()]).unwrap();
    (dir, engine)
}

/// True when `first`'s row is drawn above `second`'s.
fn above(vcx: &mut VisualTestContext, first: &str, second: &str) -> bool {
    let y = |vcx: &mut VisualTestContext, tag: &str| {
        vcx.debug_bounds(crate::tests::ui_automation::leak(format!(
            "tag-manager-row-{tag}"
        )))
        .unwrap_or_else(|| panic!("row {tag} must render"))
        .origin
        .y
    };
    y(vcx, first) < y(vcx, second)
}

fn sort(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> TagSortOrder {
    view.update(vcx, |app, _| app.tag_manager.sort)
}

#[core::prelude::v1::test]
fn tags_header_filter_and_sort() {
    let (dir, engine) = two_keywords("tm_header");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    assert!(
        vcx.debug_bounds("page-meta-text:2 keywords · 67% of library tagged")
            .is_some(),
        "the meta counts keywords and the tagged share of the library (2 of 3)"
    );
    assert!(
        vcx.debug_bounds("page-tab-keywords").is_some()
            && vcx.debug_bounds("page-tab-count-text:keywords 2").is_some(),
        "one Keywords tab with the keyword count"
    );
    for gone in ["header-filter", "sort-menu-button", "tag-sort-az-btn"] {
        assert!(
            vcx.debug_bounds(gone).is_none(),
            "the media filter/sort and the old segmented sort are not on the page: {gone}"
        );
    }
    let search = vcx
        .debug_bounds("tag-manager-search")
        .expect("filter field");
    assert_eq!(
        f32::from(search.size.width),
        200.0,
        "the filter is 200 wide"
    );
    assert_eq!(f32::from(search.size.height), 32.0, "and 32 high");
    let tabs = vcx.debug_bounds("page-tabs").unwrap();
    assert!(
        search.origin.y > tabs.origin.y
            && search.origin.y + search.size.height < tabs.origin.y + tabs.size.height,
        "the filter sits on the tab row"
    );
    assert!(
        vcx.debug_bounds("placeholder-text:Filter tags").is_some(),
        "the empty filter reads Filter tags"
    );

    // Mouse: A–Z (the default) puts apple first; Most used puts zebra first.
    assert_eq!(sort(&view, vcx), TagSortOrder::Alphabetical);
    assert!(vcx.debug_bounds("tag-sort-text:A–Z ▾").is_some());
    assert!(above(vcx, "apple", "zebra"));
    click(vcx, "tag-sort-menu");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-sort-menu-card").is_some(),
        "the menu opens"
    );
    for item in ["tag-sort-most-used", "tag-sort-az"] {
        assert!(vcx.debug_bounds(item).is_some(), "the menu offers {item}");
    }
    click(vcx, "tag-sort-most-used");
    render_ui(vcx);
    assert_eq!(sort(&view, vcx), TagSortOrder::CountDesc);
    assert!(
        vcx.debug_bounds("tag-sort-menu-card").is_none(),
        "picking closes it"
    );
    assert!(vcx.debug_bounds("tag-sort-text:Most used ▾").is_some());
    assert!(above(vcx, "zebra", "apple"), "Most used reorders the rows");

    // Clicking off the menu closes it without changing the order.
    click(vcx, "tag-sort-menu");
    render_ui(vcx);
    crate::tests::ui_automation::click_point(vcx, 600.0, 600.0);
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-sort-menu-card").is_none(),
        "the backdrop dismisses"
    );
    assert_eq!(sort(&view, vcx), TagSortOrder::CountDesc);

    // Keyboard: ⌘K Sort Tags opens it on the current order; ↓ ↵ picks A–Z.
    crate::tests::duplicates::run_palette_command(vcx, &view, "Sort Tags");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-sort-menu-card").is_some(),
        "⌘K opens the menu"
    );
    keystrokes(vcx, "down enter");
    render_ui(vcx);
    assert_eq!(
        sort(&view, vcx),
        TagSortOrder::Alphabetical,
        "↓ ↵ picks A–Z"
    );
    assert!(above(vcx, "apple", "zebra"));
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.focused.is_none(),
            "arrows in the menu do not move the row focus"
        )
    });
    // Esc closes the menu and leaves the order.
    crate::tests::duplicates::run_palette_command(vcx, &view, "Sort Tags");
    keystrokes(vcx, "up escape");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-sort-menu-card").is_none(),
        "Esc closes it"
    );
    assert_eq!(sort(&view, vcx), TagSortOrder::Alphabetical);

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tags_header_meta_without_keywords() {
    let (dir, engine) = seed_library("tm_header_empty", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    assert!(
        vcx.debug_bounds("page-meta-text:0 keywords · 0% of library tagged")
            .is_some(),
        "an untagged library reads 0 keywords · 0%"
    );
    assert!(vcx.debug_bounds("page-tab-count-text:keywords 0").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tags_page_esc_clears_checks_then_the_filter() {
    let (dir, engine) = two_keywords("tm_header_esc");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, "tag-manager-checkbox-zebra");
    keystrokes(vcx, "z");
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.selected.is_empty(),
            "Esc first clears the checks"
        );
        assert_eq!(app.tag_manager.search, "z", "and keeps the filter");
    });
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.search.is_empty(),
            "then Esc clears the filter"
        );
        assert!(app.tags_page_shown(), "and the page stays");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tags_header_meta_before_totals_load() {
    let (dir, engine) = two_keywords("tm_header_loading");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);
    view.update(vcx, |app, cx| {
        app.total_library_count = 0;
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("page-meta-text:2 keywords").is_some(),
        "without the library total the meta shows the keyword count alone"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn menu_open(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.tag_manager.sort_menu.is_some())
}

#[core::prelude::v1::test]
fn tag_sort_menu_closes_when_the_page_or_keys_move_elsewhere() {
    let (dir, engine) = two_keywords("tm_header_leave");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, "tag-sort-menu");
    keystrokes(vcx, "secondary-[");
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| assert!(app.tags_page_shown(), "⌘] returns"));
    assert!(
        !menu_open(&view, vcx),
        "Back closes the menu; Forward does not reopen it"
    );
    assert!(vcx.debug_bounds("tag-sort-menu-card").is_none());

    click(vcx, "tag-sort-menu");
    keystrokes(vcx, "secondary-k");
    assert!(
        !menu_open(&view, vcx),
        "⌘K closes the menu under the palette"
    );
    keystrokes(vcx, "escape");

    click(vcx, "tag-sort-menu");
    keystrokes(vcx, "secondary-f");
    assert!(
        !menu_open(&view, vcx),
        "⌘F gives the keys to the sidebar search"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn tag_sort_menu_swallows_page_keys() {
    let (dir, engine) = two_keywords("tm_header_swallow");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    click(vcx, "tag-manager-checkbox-zebra");
    click(vcx, "tag-manager-checkbox-apple");
    click(vcx, "tag-sort-menu");
    keystrokes(vcx, "a p secondary-backspace secondary-a");
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.sort_menu.is_some(), "the menu stays open");
        assert!(app.tag_manager.search.is_empty(), "letters do not filter");
        assert_eq!(
            app.tag_manager.selected.len(),
            2,
            "⌘A does not change the checks"
        );
        assert_eq!(app.library_tag_counts.len(), 2, "⌘⌫ deletes nothing");
    });

    let _ = std::fs::remove_dir_all(&dir);
}
