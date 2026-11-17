//! Merge into… (ORGANISE): a single-tag target picker on the
//! detail panel, the row menu and the palette. The merged name stays as an
//! alias of the target, in the same undo step.

use super::duplicates::run_palette_command;
use super::tag_manager::{checkbox, count, open};
use super::tag_manager_sidecars::{sidecar_library, sidecar_tags};
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library, type_text,
};

fn picker_open(view: &gpui::Entity<crate::LuminaApp>, vcx: &mut gpui::VisualTestContext) -> bool {
    view.update(vcx, |app, _| app.tag_manager.merge.is_some())
}

#[core::prelude::v1::test]
fn merge_into_row_picks_target_by_keyboard() {
    // Item 0: sand, sea, shore, unique (with a sidecar); item 1: beach.
    let (dir, engine, item) = sidecar_library("tm_merge_into", &["sand", "sea", "shore", "unique"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    open(&view, vcx);
    click(vcx, "tag-manager-row-sand");
    render_ui(vcx);

    click(vcx, "tag-detail-merge");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("tag-detail-merge-field").is_some(),
        "Merge into… opens the target field"
    );
    for tag in ["beach", "sea", "shore", "unique"] {
        assert!(
            vcx.debug_bounds(crate::tests::ui_automation::leak(format!(
                "tag-detail-merge-option-{tag}"
            )))
            .is_some(),
            "{tag} is offered as a target"
        );
    }
    assert!(
        vcx.debug_bounds("tag-detail-merge-option-sand").is_none(),
        "a keyword is never offered as its own target"
    );

    // Typing filters the targets, not the tag list.
    type_text(vcx, "s");
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.search.is_empty(),
            "picker typing must not filter the list"
        );
        assert_eq!(
            app.tag_merge_options(),
            ["sea", "shore"],
            "typing filters the targets"
        );
    });
    assert!(vcx.debug_bounds("tag-detail-merge-option-beach").is_none());

    // ↓ moves to shore, ↑ back to sea, ↓ again; ↵ merges.
    keystrokes(vcx, "down up down enter");
    vcx.run_until_parked();
    assert!(!picker_open(&view, vcx), "↵ closes the picker");
    assert_eq!(count(&view, vcx, "sand"), None, "the source is gone");
    assert_eq!(count(&view, vcx, "shore"), Some(1), "its items moved");
    assert_eq!(count(&view, vcx, "sea"), Some(1), "↑↓ chose shore, not sea");
    assert_eq!(
        sidecar_tags(&item),
        ["sea", "shore", "unique"],
        "the merge rewrites the sidecar"
    );
    assert_eq!(
        engine.resolve_alias("sand").unwrap(),
        "shore",
        "sand is an alias"
    );
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert_eq!(app.tag_manager.focused.as_deref(), Some("shore"));
    });
    assert!(
        vcx.debug_bounds("tag-detail-alias-sand").is_some(),
        "the target's ALIASES shows the merged name"
    );

    // One ⌘Z puts the keyword back and removes the alias.
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-z");
    vcx.run_until_parked();
    assert_eq!(count(&view, vcx, "sand"), Some(1), "⌘Z restores sand");
    assert!(
        engine.aliases().unwrap().is_empty(),
        "and removes its alias"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.keywords.aliases.is_empty(),
            "the cached aliases follow undo"
        )
    });
    assert_eq!(sidecar_tags(&item), ["sand", "sea", "shore", "unique"]);

    // Esc cancels without merging; background keys are absorbed.
    click(vcx, "tag-manager-row-sand");
    click(vcx, "tag-detail-merge");
    keystrokes(vcx, "space backspace escape");
    assert!(!picker_open(&view, vcx), "Esc closes the picker");
    assert_eq!(count(&view, vcx, "sand"), Some(1), "Esc merges nothing");
    view.update(vcx, |app, _| {
        assert!(
            app.tag_manager.selected.is_empty(),
            "Space did not check the row"
        )
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn merge_into_from_row_menu_and_palette() {
    let (dir, engine) = seed_library("tm_merge_menu", 2);
    let ids: Vec<_> = engine.get_item_slice(0..2).iter().map(|i| i.id).collect();
    engine.add_tags_batch(&ids[..1], &["trip".into()]).unwrap();
    engine
        .add_tags_batch(&ids[1..], &["trip/rome".into(), "places".into()])
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    open(&view, vcx);

    click(vcx, "tag-row-menu-places");
    click(vcx, "tag-row-menu//merge");
    assert!(picker_open(&view, vcx), "the row menu offers Merge into…");
    keystrokes(vcx, "escape");

    click(vcx, "tag-manager-row-trip");
    run_palette_command(vcx, &view, "Merge Tag Into");
    assert!(picker_open(&view, vcx), "the palette opens the picker");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.tag_merge_options(),
            ["places"],
            "neither the keyword nor one below it is a target"
        )
    });
    keystrokes(vcx, "enter");
    assert_eq!(
        count(&view, vcx, "places/rome"),
        Some(1),
        "children move too"
    );
    assert!(
        engine.aliases().unwrap().is_empty(),
        "a parent with children keeps no alias"
    );

    // A refused merge reports in the banner and keeps the picker open.
    click(vcx, "tag-manager-row-places");
    click(vcx, "tag-detail-merge");
    view.update(vcx, |app, cx| {
        app.merge_tag_into("places".into(), "places/rome".into(), cx)
    });
    view.update(vcx, |app, _| {
        let error = app.error_message.clone().unwrap_or_default();
        assert!(error.contains("below it"), "the banner says why: {error}");
        assert!(
            app.tag_manager.merge.is_some(),
            "the picker stays for a retry"
        );
    });
    assert_eq!(count(&view, vcx, "places"), Some(2), "nothing merged");

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn bulk_merge_aliases_every_source_and_undoes_in_one_step() {
    // beach is on both items, so the checked tags fold into it.
    let (dir, engine, _item) = sidecar_library("tm_merge_bulk", &["beach", "coast", "sea"]);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    open(&view, vcx);
    for tag in ["beach", "coast", "sea"] {
        click(vcx, checkbox(tag));
    }
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-m");
    vcx.run_until_parked();
    let pair = |a: &str| (a.to_string(), "beach".to_string());
    assert_eq!(
        engine.aliases().unwrap(),
        [pair("coast"), pair("sea")],
        "⌘M keeps one alias per merged source"
    );
    view.update(vcx, |app, _| {
        assert_eq!(
            app.aliases_of("beach"),
            ["coast", "sea"],
            "the cache follows"
        )
    });

    keystrokes(vcx, "secondary-z");
    vcx.run_until_parked();
    assert!(
        engine.aliases().unwrap().is_empty(),
        "one ⌘Z removes every alias the merge added"
    );
    assert_eq!(count(&view, vcx, "coast"), Some(1));
    assert_eq!(count(&view, vcx, "sea"), Some(1));

    let _ = std::fs::remove_dir_all(&dir);
}
