//! Compare against the features around it: the viewer opened from compare
//! refuses what would change the view behind compare (keys, palette and menu
//! bar alike), menu items wait for an open palette, and a library switch
//! closes compare.

use std::sync::Arc;

use crate::app::command_palette::{CommandId, PaletteTarget};
use crate::tests::culling::{card, ids};
use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::*;
use lumina_core::Flag;

/// Clicks the first card and extends the selection to `n` with ⇧→, then C.
fn open_compare_on_first(vcx: &mut VisualTestContext, ids: &[lumina_core::MediaId], n: usize) {
    click(vcx, card(ids[0]));
    for _ in 1..n {
        keystrokes(vcx, "shift-right");
    }
    keystrokes(vcx, "c");
}

/// The palette commands that would play, move or trash the view.
const VIEW_COMMANDS: [(&str, CommandId); 6] = [
    ("slideshow", CommandId::Slideshow),
    ("show map", CommandId::ShowMap),
    ("back", CommandId::Back),
    ("similar shots", CommandId::ShowSimilarShots),
    ("exact copies", CommandId::ShowExactCopies),
    ("delete rejected", CommandId::DeleteRejected),
];

/// Which of `VIEW_COMMANDS` the palette lists, each found by typing its name.
fn listed(view: &Entity<LuminaApp>, vcx: &mut VisualTestContext) -> Vec<CommandId> {
    view.update(vcx, |app, _| {
        let found = VIEW_COMMANDS
            .iter()
            .filter(|(query, id)| {
                app.command_query = query.to_string();
                app.palette_rows()
                    .iter()
                    .any(|row| row.target == PaletteTarget::Command(*id))
            })
            .map(|(_, id)| *id)
            .collect();
        app.command_query.clear();
        found
    })
}

/// The viewer opened from compare shows one compared item over compare,
/// which sits on the view. Back/Forward, Slideshow, Show Map, Similar shots
/// and Delete Rejected… would play, move or trash that view and leave
/// compare on top of another, so the keys refuse them, the palette leaves
/// them out and the menu bar disables them: the same rule for each.
#[core::prelude::v1::test]
fn viewer_from_compare_refuses_what_would_change_the_view_behind_it() {
    let (dir, engine) = seed_library("compare_view_guards", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    let ids = ids(&view, vcx);
    // A reject for Delete Rejected… and a trail for Back.
    engine.set_flags(&[(ids[3], Flag::Reject)]).unwrap();
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Trash, cx);
        app.select_album(AlbumSelection::AllLibrary, cx);
    });
    render_ui(vcx);
    assert_eq!(
        listed(&view, vcx),
        VIEW_COMMANDS.map(|(_, id)| id).to_vec(),
        "outside compare the palette lists every one (the probe is not vacuous)"
    );

    open_compare_on_first(vcx, &ids, 2);
    keystrokes(vcx, "enter");
    focus_app(vcx, &view);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.compare.is_some() && app.is_fullscreen_viewer);
    });

    // The menu bar: an item is enabled when the root handles its action.
    let menu: [(&str, Box<dyn gpui::Action>); 5] = [
        ("Slideshow", Box::new(crate::actions::Slideshow)),
        ("Show Map", Box::new(crate::actions::ShowMap)),
        ("Back", Box::new(crate::actions::GoBack)),
        ("Forward", Box::new(crate::actions::GoForward)),
        ("Delete Rejected…", Box::new(crate::actions::DeleteRejected)),
    ];
    let enabled: Vec<&str> = menu
        .iter()
        .filter(|(_, action)| vcx.update(|window, cx| window.is_action_available(&**action, cx)))
        .map(|(name, _)| *name)
        .collect();
    assert!(
        vcx.update(|window, cx| window.is_action_available(&crate::actions::ToggleInspector, cx)),
        "the menu probe sees enabled items"
    );
    assert!(
        enabled.is_empty(),
        "menu items that would change the view behind compare are disabled: {enabled:?}"
    );
    assert!(
        listed(&view, vcx).is_empty(),
        "the palette leaves out the same commands: {:?}",
        listed(&view, vcx)
    );

    // The keys and the palette's own run path refuse them too.
    let unchanged = |view: &Entity<LuminaApp>, vcx: &mut VisualTestContext, what: &str| {
        render_ui(vcx);
        view.update(vcx, |app, _| {
            let state = (
                app.compare.is_some(),
                app.is_fullscreen_viewer,
                app.selection.clone(),
                app.selected_item.as_ref().map(|i| i.id),
                app.delete_confirm_state.is_some(),
                app.slideshow.is_some(),
            );
            assert_eq!(
                state,
                (
                    true,
                    true,
                    AlbumSelection::AllLibrary,
                    Some(ids[0]),
                    false,
                    false
                ),
                "{what} leaves the viewer, compare and the view behind it alone"
            );
        });
    };
    keystrokes(vcx, "secondary-[");
    unchanged(&view, vcx, "⌘[");
    keystrokes(vcx, "secondary-]");
    unchanged(&view, vcx, "⌘]");
    for (_, id) in VIEW_COMMANDS {
        view.update(vcx, |app, cx| app.execute_command(id, cx));
        unchanged(&view, vcx, &format!("{id:?} run from the palette"));
    }
    view.update(vcx, |app, cx| app.execute_command(CommandId::Forward, cx));
    unchanged(&view, vcx, "Forward run from the palette");

    keystrokes(vcx, "escape");
    assert!(
        vcx.debug_bounds("compare-view").is_some(),
        "Esc returns to compare over the same view"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The menu bar stays reachable while the ⌘K palette is open. View >
/// Compare, Slideshow and Show Map must not start under it, where the
/// palette's keys would act on what they show.
#[core::prelude::v1::test]
fn menu_compare_slideshow_and_map_wait_for_the_palette() {
    let (dir, engine) = seed_library("compare_under_palette", 4);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    click(vcx, card(ids[0]));
    keystrokes(vcx, "shift-right");
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k");
    assert!(view.update(vcx, |app, _| app.command_palette_open));

    type Dispatch = fn(&mut VisualTestContext);
    let menu: [(&str, Dispatch); 3] = [
        ("Compare", |v| v.dispatch_action(crate::actions::Compare)),
        ("Slideshow", |v| {
            v.dispatch_action(crate::actions::Slideshow)
        }),
        ("Show Map", |v| v.dispatch_action(crate::actions::ShowMap)),
    ];
    let mut started = Vec::new();
    for (name, dispatch) in menu {
        dispatch(vcx);
        render_ui(vcx);
        view.update(vcx, |app, cx| {
            if app.compare.is_some()
                || app.slideshow.is_some()
                || app.map.shown
                || app.selection != AlbumSelection::AllLibrary
            {
                started.push(name);
                // Undone, so each item is tried on its own.
                app.close_compare(cx);
                app.drop_slideshow(cx);
                app.map.shown = false;
                app.select_album(AlbumSelection::AllLibrary, cx);
            }
        });
    }
    assert!(
        started.is_empty(),
        "nothing starts under the palette: {started:?}"
    );
    view.update(vcx, |app, _| {
        assert!(app.command_palette_open, "the palette stays");
        assert_eq!(app.selected_indices.len(), 2, "the selection is kept");
    });

    // Closed, the same menu item works.
    keystrokes(vcx, "escape");
    vcx.dispatch_action(crate::actions::Compare);
    render_ui(vcx);
    assert!(view.update(vcx, |app, _| app.compare.is_some()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Compare holds the old library's items and textures: opening another
/// library closes it and evicts them.
#[core::prelude::v1::test]
fn opening_another_library_closes_compare() {
    let (dir, engine) = seed_library("compare_switch", 3);
    let (other, _other_engine) = seed_library("compare_switch_other", 3);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let ids = ids(&view, vcx);
    open_compare_on_first(vcx, &ids, 2);
    let generation = view.update(vcx, |app, _| {
        app.compare.as_ref().expect("C opens compare").generation
    });
    let image = Arc::new(gpui::RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::new(4, 4),
    )]));
    view.update(vcx, |app, cx| {
        app.land_compare_image(generation, ids[0], image.clone(), cx)
    });
    render_ui(vcx);

    view.update(vcx, |app, cx| app.open_library_inner(other.clone(), cx));
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.compare.is_none(), "a library switch closes compare");
        assert!(
            app.selected_item.is_none() && app.selected_indices.is_empty(),
            "no old id is selected in the new library"
        );
    });
    assert!(vcx.debug_bounds("compare-view").is_none());
    assert_eq!(
        Arc::strong_count(&image),
        1,
        "the old library's texture is evicted"
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&other);
}
