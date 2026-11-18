//! Tags page keyword table rows: 44 px rows, the 190 px
//! usage track, alias counts, the focused row's accent edge, the capped
//! hierarchy indent and the row `…` menu by mouse, keyboard and palette.

use super::tag_manager::{checkbox, count, open};
use crate::tests::duplicates::run_palette_command;
use crate::tests::helpers::{
    app_with_library, click, click_point, focus_app, keystrokes, leak, seed_library,
};
use crate::LuminaApp;
use gpui::{Entity, VisualTestContext};

/// beach on 3 items with 2 aliases, sea on 1 with 1 alias, and a keyword
/// seven levels deep on 1.
fn row_library(
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
    std::sync::Arc<lumina_core::LuminaEngine>,
) {
    let (dir, engine) = seed_library(prefix, 3);
    let ids: Vec<_> = engine.get_item_slice(0..3).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids, &["beach".into()]).unwrap();
    engine.add_tags_batch(&ids[..1], &["sea".into()]).unwrap();
    engine
        .add_tags_batch(&ids[..1], &["a/b/c/d/e/f/g".into()])
        .unwrap();
    engine.set_alias("shore", "beach").unwrap();
    engine.set_alias("strand", "beach").unwrap();
    engine.set_alias("ocean", "sea").unwrap();
    (dir, engine)
}

fn bounds(vcx: &mut VisualTestContext, selector: String) -> gpui::Bounds<gpui::Pixels> {
    vcx.debug_bounds(leak(selector.clone()))
        .unwrap_or_else(|| panic!("{selector} must render"))
}

fn width(b: gpui::Bounds<gpui::Pixels>) -> f32 {
    f32::from(b.size.width)
}

fn menu_open(vcx: &mut VisualTestContext) -> bool {
    vcx.debug_bounds("tag-row-menu//card").is_some()
}

fn editing(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> (Option<String>, bool) {
    view.update(vcx, |app, _| {
        (app.tag_manager.editing.clone(), app.tag_manager.moving)
    })
}

/// Opens the Tags page on [`row_library`] in a 1440×900 window.
fn open_wide(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) {
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    open(view, vcx);
}

#[core::prelude::v1::test]
fn tag_rows_scale_usage_count_aliases_and_mark_focus() {
    let (_dir, engine) = row_library("tm_rows");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_wide(&view, vcx);

    let track = bounds(vcx, "tag-manager-usage-beach".into());
    let full = bounds(vcx, "tag-manager-usage-fill-beach".into());
    let third = bounds(vcx, "tag-manager-usage-fill-sea".into());
    assert_eq!(
        width(full),
        width(track),
        "the most used tag fills the track"
    );
    assert!(
        (width(third) - width(track) / 3.0).abs() < 0.5,
        "fills scale to the most used tag: {third:?}"
    );

    assert!(
        vcx.debug_bounds("tag-row-aliases-beach").is_some()
            && vcx
                .debug_bounds("tag-row-aliases-text:beach +2 aliases")
                .is_some(),
        "two aliases read +2 aliases"
    );
    assert!(
        vcx.debug_bounds("tag-row-aliases-text:sea +1 alias")
            .is_some(),
        "one alias reads +1 alias"
    );
    assert!(
        vcx.debug_bounds("tag-row-aliases-a/b/c/d/e/f/g").is_none(),
        "a keyword without aliases shows no alias count"
    );

    // Hierarchy: the indent stops growing, so the usage column lines up.
    let deep = "a/b/c/d/e/f/g";
    let deep_usage = bounds(vcx, format!("tag-manager-usage-{deep}"));
    assert_eq!(
        deep_usage.origin.x, track.origin.x,
        "deep and shallow rows keep the usage column at the same x"
    );
    let deep_indent = bounds(vcx, "tag-row-indent-6".into());
    assert!(
        width(deep_indent) <= 64.0,
        "the indent is capped: {deep_indent:?}"
    );

    // The focused row has the accent edge; others do not.
    click(vcx, "tag-manager-row-beach");
    assert!(
        vcx.debug_bounds("tag-row-focus-beach").is_some(),
        "the focused row draws its accent left border"
    );
    assert!(vcx.debug_bounds("tag-row-focus-sea").is_none());

    // Checkbox still toggles without moving focus.
    click(vcx, checkbox("sea"));
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.selected.contains("sea"), "checkbox checks");
        assert_eq!(app.tag_manager.focused.as_deref(), Some("beach"));
    });
    click(vcx, checkbox("sea"));
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.selected.is_empty(), "and unchecks")
    });
}

#[core::prelude::v1::test]
fn tag_row_menu_by_mouse() {
    let (_dir, engine) = row_library("tm_row_menu");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_wide(&view, vcx);

    let button = bounds(vcx, "tag-row-menu-sea".into());
    assert_eq!((width(button), f32::from(button.size.height)), (32.0, 28.0));
    click(vcx, "tag-row-menu-sea");
    assert!(menu_open(vcx), "the … button opens the row menu");
    for item in [
        "tag-row-menu//rename",
        "tag-row-menu//merge",
        "tag-row-menu//move",
        "tag-row-menu//delete",
    ] {
        assert!(vcx.debug_bounds(item).is_some(), "the menu offers {item}");
    }
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_manager.focused.as_deref(),
            Some("sea"),
            "the menu acts on its row, which takes focus"
        )
    });

    // The backdrop dismisses without acting.
    click_point(vcx, 700.0, 860.0);
    assert!(!menu_open(vcx), "clicking off the menu closes it");
    assert_eq!(editing(&view, vcx), (None, false));

    click(vcx, "tag-row-menu-sea");
    click(vcx, "tag-row-menu//rename");
    assert!(!menu_open(vcx), "picking closes the menu");
    assert_eq!(editing(&view, vcx), (Some("sea".into()), false));
    assert!(vcx.debug_bounds("tag-manager-rename-field").is_some());

    click(vcx, "tag-row-menu-sea");
    click(vcx, "tag-row-menu//move");
    assert_eq!(editing(&view, vcx), (Some("sea".into()), true), "Move to…");

    click(vcx, "tag-row-menu-sea");
    click(vcx, "tag-row-menu//delete");
    assert_eq!(count(&view, vcx, "sea"), None, "Delete removes the tag");
    focus_app(vcx, &view);
    keystrokes(vcx, "cmd-z");
    assert_eq!(count(&view, vcx, "sea"), Some(1), "and ⌘Z restores it");
}

#[core::prelude::v1::test]
fn tag_row_menu_by_keyboard_and_palette() {
    let (_dir, engine) = row_library("tm_row_menu_keys");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_wide(&view, vcx);
    focus_app(vcx, &view);

    keystrokes(vcx, "shift-f10");
    assert!(!menu_open(vcx), "no focused row, no menu");

    click(vcx, "tag-manager-row-sea");
    focus_app(vcx, &view);
    keystrokes(vcx, "shift-f10");
    assert!(menu_open(vcx), "⇧F10 opens the focused row's menu");
    assert!(vcx.debug_bounds("tag-row-menu//card-text:sea").is_some());
    keystrokes(vcx, "escape");
    assert!(!menu_open(vcx), "Esc closes it");
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("sea"));
    });

    // ↓ ↓ ↵ picks Move to…; keys go to the menu, not the list.
    keystrokes(vcx, "shift-f10");
    keystrokes(vcx, "down down");
    keystrokes(vcx, "enter");
    assert!(!menu_open(vcx));
    assert_eq!(editing(&view, vcx), (Some("sea".into()), true));
    view.update(vcx, |app, _| {
        assert_eq!(app.tag_manager.focused.as_deref(), Some("sea"));
        app.cancel_tag_manager_prompts();
    });

    // ↓ past the end stays on Delete; ↑ ×4 back to Rename; ↵ renames.
    keystrokes(vcx, "shift-f10");
    keystrokes(vcx, "down down down down down up up up up");
    keystrokes(vcx, "enter");
    assert_eq!(editing(&view, vcx), (Some("sea".into()), false));
    view.update(vcx, |app, _| app.cancel_tag_manager_prompts());

    // Every item has a palette equivalent.
    run_palette_command(vcx, &view, "Rename Keyword");
    assert_eq!(editing(&view, vcx), (Some("sea".into()), false));
    view.update(vcx, |app, _| app.cancel_tag_manager_prompts());
    run_palette_command(vcx, &view, "Delete Keyword");
    assert_eq!(count(&view, vcx, "sea"), None, "palette Delete Keyword");
}

#[core::prelude::v1::test]
fn tag_row_menu_takes_typing_and_closes_with_the_page() {
    let (_dir, engine) = row_library("tm_row_menu_guard");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open_wide(&view, vcx);
    let row_menu = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.tag_manager.row_menu.clone())
    };

    click(vcx, "tag-row-menu-sea");
    focus_app(vcx, &view);
    keystrokes(vcx, "a s");
    view.update(vcx, |app, _| {
        assert!(app.tag_manager.search.is_empty(), "letters do not filter");
    });
    assert_eq!(
        row_menu(&view, vcx),
        Some(("sea".into(), 0)),
        "the menu stays open on its row"
    );

    keystrokes(vcx, "secondary-[");
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| assert!(app.tags_page_shown(), "⌘] returns"));
    assert_eq!(row_menu(&view, vcx), None, "Back closes the menu");
    assert!(!menu_open(vcx));

    click(vcx, "tag-row-menu-sea");
    keystrokes(vcx, "secondary-k");
    assert_eq!(row_menu(&view, vcx), None, "⌘K closes it under the palette");
    keystrokes(vcx, "escape");

    click(vcx, "tag-row-menu-sea");
    keystrokes(vcx, "secondary-f");
    assert_eq!(
        row_menu(&view, vcx),
        None,
        "⌘F gives the keys to the search"
    );
}

#[core::prelude::v1::test]
fn shift_f10_scrolls_the_focused_row_into_view() {
    let (_dir, engine) = seed_library("tm_row_menu_scroll", 1);
    let id = engine.get_item_slice(0..1)[0].id;
    let tags: Vec<String> = (0..40).map(|i| format!("tag{i:02}")).collect();
    engine.add_tags_batch(&[id], &tags).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(640.0)));
    open(&view, vcx);
    view.update(vcx, |app, cx| {
        app.tag_manager.search_focused = false;
        app.focus_tag_manager_tag("tag39".into(), cx);
    });
    focus_app(vcx, &view);
    crate::tests::helpers::render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-manager-row-tag39").is_none(),
        "the focused row starts off screen (rows are virtualised)"
    );

    keystrokes(vcx, "shift-f10");
    assert!(
        vcx.debug_bounds("tag-manager-row-tag39").is_some(),
        "⇧F10 scrolls the focused row into view"
    );
    assert!(menu_open(vcx), "and draws its menu");
}

/// Names keep about 160 px. The usage track drops first, then
/// the count; the widths come from the window, sidebar and 359.5 panel.
#[core::prelude::v1::test]
fn narrow_windows_drop_the_usage_track_then_the_count() {
    let (_dir, engine) = seed_library("tm_rows_narrow", 2);
    let ids: Vec<_> = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids, &["beach".into()]).unwrap();
    engine.add_tags_batch(&ids[..1], &["sea".into()]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    open(&view, vcx);

    let check = |vcx: &mut VisualTestContext, window: f32, track: bool, count: bool| {
        vcx.simulate_resize(gpui::size(gpui::px(window), gpui::px(700.0)));
        crate::tests::helpers::render_ui(vcx);
        let list = bounds(vcx, "tag-manager-list".into());
        let row = bounds(vcx, "tag-manager-row-beach".into());
        let name = bounds(vcx, "tag-manager-name-beach".into());
        assert!(
            width(name) >= 160.0,
            "at {window} px the name keeps 160 px: {name:?}"
        );
        assert!(
            row.origin.x + row.size.width <= list.origin.x + list.size.width,
            "at {window} px the row does not overflow the list: {row:?} {list:?}"
        );
        assert!(vcx.debug_bounds("tag-row-menu-beach").is_some());
        assert_eq!(
            vcx.debug_bounds("tag-manager-usage-beach").is_some(),
            track,
            "at {window} px the usage track shows: {track}"
        );
        assert_eq!(
            vcx.debug_bounds("tag-manager-count-beach").is_some(),
            count,
            "at {window} px the count shows: {count}"
        );
    };
    check(vcx, 1440.0, true, true);
    check(vcx, 1100.0, false, true);
    check(vcx, 960.0, false, false);
}
