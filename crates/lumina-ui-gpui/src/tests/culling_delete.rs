//! Finishing a cull (plan 5 B3): Delete Rejected… sends exactly the current
//! view's rejects to the Delete dialog, and a smart album on the flag
//! recounts after a flag key.

use super::culling::{card, ids};
use crate::tests::ui_automation::{
    app_with_library, click, keystrokes, render_ui, seed_library, type_text,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::{
    Flag, LuminaEngine, MatchMode, MediaId, RuleField, RuleOperator, RuleValue, SmartAlbumRule,
};

fn sorted(mut ids: Vec<MediaId>) -> Vec<MediaId> {
    ids.sort_by_key(|id| id.0);
    ids
}

fn trashed(engine: &LuminaEngine, ids: &[MediaId]) -> Vec<MediaId> {
    sorted(
        ids.iter()
            .copied()
            .filter(|id| {
                engine
                    .get_item_by_id(*id)
                    .is_some_and(|item| item.is_trashed)
            })
            .collect(),
    )
}

fn toast(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<String> {
    view.update(vcx, |app, _| app.toast.current().map(|t| t.message.clone()))
}

fn dialog_ids(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Option<Vec<MediaId>> {
    view.update(vcx, |app, _| {
        app.delete_confirm_state.as_ref().map(|st| {
            assert_eq!(st.item_count, st.item_ids.len(), "the count is the ids'");
            sorted(st.item_ids.clone())
        })
    })
}

#[core::prelude::v1::test]
fn delete_rejected_opens_dialog_with_only_rejects() {
    let (dir, engine) = seed_library("cull_delete_rejected", 5);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    let album = engine.create_album("Trip").unwrap();
    engine.add_to_album(album.id, &all[..4]).unwrap();
    // In the album: two rejects, a pick, an unflagged item. Outside it: one
    // more reject that must never reach the dialog.
    engine
        .set_flags(&[
            (all[0], Flag::Reject),
            (all[1], Flag::Pick),
            (all[2], Flag::Reject),
            (all[4], Flag::Reject),
        ])
        .unwrap();
    let album_rejects = sorted(vec![all[0], all[2]]);
    view.update(vcx, |app, cx| {
        app.item_flags = engine.get_flags_map();
        app.select_album(AlbumSelection::Album(album.id), cx);
    });
    render_ui(vcx);

    // Keyboard path: ⌘K, the palette entry, ↵.
    keystrokes(vcx, "cmd-k");
    type_text(vcx, "delete rejected");
    keystrokes(vcx, "enter");
    render_ui(vcx);
    assert_eq!(
        dialog_ids(&view, vcx),
        Some(album_rejects.clone()),
        "the dialog holds the album's rejects only, not the one outside it"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "Delete Rejected never writes is:reject into the search box"
        );
        assert_eq!(app.selection, AlbumSelection::Album(album.id));
    });

    keystrokes(vcx, "escape");
    assert_eq!(dialog_ids(&view, vcx), None, "Esc closes the dialog");
    for id in &all {
        assert!(
            engine.get_item_by_id(*id).is_some(),
            "Esc removes nothing from the catalog"
        );
    }
    assert!(trashed(&engine, &all).is_empty(), "Esc deletes nothing");
    assert_eq!(engine.get_flags_map().len(), 4, "flags are untouched");

    // The File menu action opens the same dialog; ↵ is Move to Trash.
    vcx.dispatch_action(crate::actions::DeleteRejected);
    render_ui(vcx);
    assert_eq!(dialog_ids(&view, vcx), Some(album_rejects.clone()));
    keystrokes(vcx, "enter");
    assert_eq!(
        trashed(&engine, &all),
        album_rejects,
        "confirming trashes exactly the album's rejects"
    );
    assert_eq!(dialog_ids(&view, vcx), None);

    keystrokes(vcx, "cmd-z");
    assert!(
        trashed(&engine, &all).is_empty(),
        "one undo brings both rejects back"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn delete_rejected_with_none_shows_toast() {
    let (dir, engine) = seed_library("cull_delete_none", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    engine.set_flags(&[(all[0], Flag::Pick)]).unwrap();

    vcx.dispatch_action(crate::actions::DeleteRejected);
    render_ui(vcx);
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some("No rejected items here"),
        "with no rejects a toast says so"
    );
    assert_eq!(dialog_ids(&view, vcx), None, "and no dialog opens");
    assert!(trashed(&engine, &all).is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn smart_album_with_flag_rule_recounts_after_flag_key() {
    let (dir, engine) = seed_library("cull_flag_rule_count", 3);
    let picks = engine
        .create_smart_album(
            "Picks",
            "sparkles",
            MatchMode::All,
            &[SmartAlbumRule {
                field: RuleField::Flag,
                operator: RuleOperator::Equals,
                value: RuleValue::Integer(Flag::Pick as i64),
            }],
        )
        .expect("create flag smart album")
        .id;
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let all = ids(&view, vcx);
    let count = |vcx: &mut VisualTestContext| {
        view.update(vcx, |app, _| app.user_album_counts.get(&picks).copied())
    };
    assert_eq!(count(vcx), Some(0), "nothing is picked yet");

    click(vcx, card(all[1]));
    keystrokes(vcx, "p");
    assert_eq!(
        count(vcx),
        Some(1),
        "a flag key recounts a smart album with a Flag rule"
    );
    keystrokes(vcx, "u");
    assert_eq!(count(vcx), Some(0), "unflagging recounts it too");

    let _ = std::fs::remove_dir_all(&dir);
}

/// One reject in the library, so a lookup that is allowed to land opens
/// the dialog.
fn one_reject(prefix: &str) -> (std::path::PathBuf, std::sync::Arc<LuminaEngine>) {
    let (dir, engine) = seed_library(prefix, 3);
    let first = engine.get_item_slice(0..1)[0].id;
    engine.set_flags(&[(first, Flag::Reject)]).unwrap();
    (dir, engine)
}

#[core::prelude::v1::test]
fn delete_rejected_late_result_is_dropped() {
    let (dir, engine) = one_reject("cull_delete_late");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // The view changes before the lookup lands: its rejects are stale.
    view.update(vcx, |app, cx| {
        app.delete_rejected(cx);
        app.select_album(AlbumSelection::Untagged, cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        dialog_ids(&view, vcx),
        None,
        "a lookup for a view the user left opens no dialog"
    );

    // A modal opens before it lands: ↵ there must not reach a Delete dialog.
    view.update(vcx, |app, cx| {
        app.delete_rejected(cx);
        app.toggle_shortcuts_overlay(cx);
    });
    vcx.run_until_parked();
    assert_eq!(
        dialog_ids(&view, vcx),
        None,
        "a lookup that lands under another modal opens no dialog"
    );

    // With that modal still open, the command refuses at once.
    view.update(vcx, |app, cx| app.delete_rejected(cx));
    vcx.run_until_parked();
    assert_eq!(dialog_ids(&view, vcx), None);
    assert_eq!(
        toast(&view, vcx).as_deref(),
        Some(crate::i18n::t("album.close_dialog_first").as_ref()),
        "the command says why it did nothing"
    );

    // Control: closed, the same library does open the dialog.
    view.update(vcx, |app, cx| {
        app.close_shortcuts_overlay(cx);
        app.delete_rejected(cx);
    });
    vcx.run_until_parked();
    assert_eq!(dialog_ids(&view, vcx).map(|ids| ids.len()), Some(1));

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn delete_rejected_is_refused_in_trash_and_duplicates() {
    let (dir, engine) = one_reject("cull_delete_refused");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    for place in [AlbumSelection::Trash, AlbumSelection::Duplicates] {
        view.update(vcx, |app, cx| {
            app.select_album(place.clone(), cx);
            app.open_command_palette(cx);
            app.command_query = "delete rejected".into();
            let offered = app
                .palette_rows()
                .iter()
                .any(|row| row.title == crate::i18n::t("command.delete_rejected"));
            assert!(!offered, "{place:?}: the palette does not offer it");
            app.command_palette_open = false;
            app.command_query.clear();
        });
        vcx.dispatch_action(crate::actions::DeleteRejected);
        render_ui(vcx);
        assert_eq!(
            dialog_ids(&view, vcx),
            None,
            "{place:?}: the menu action opens no dialog"
        );
        assert!(
            toast(&view, vcx).is_some_and(|t| t.contains("not available")),
            "{place:?}: a toast says why"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
