//! Toasts, the error banner and empty states.

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::helpers::TempLibrary;
use super::helpers::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::views::toast::{ToastAction, ToastKind, TOAST_DURATION};

#[core::prelude::v1::test]
fn toast_sits_centred_over_the_grid_and_waits_while_hovered() {
    let (_dir, engine) = seed_library("states_toast", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1440.0), gpui::px(900.0)));
    view.update(vcx, |app, cx| app.show_undo_toast("Rated 1 item", cx));
    render_ui(vcx);

    let toast = vcx.debug_bounds("toast").expect("toast renders");
    let grid = vcx.debug_bounds("media-grid-surface").unwrap();
    let status = vcx.debug_bounds("status-bar").unwrap();
    let off = f32::from(toast.center().x - grid.center().x).abs();
    assert!(off < 2.0, "toast is centred over the grid, off by {off}");
    // Over the grid the zoom pill sits 24 above the status bar; the toast
    // stacks 8 above the pill's 42.
    let pill = vcx.debug_bounds("grid-zoom-pill").expect("zoom pill");
    let gap = f32::from(pill.top() - toast.bottom());
    assert!(
        (gap - 8.0).abs() < 1.0,
        "8px above the zoom pill, got {gap}"
    );
    let gap = f32::from(status.top() - toast.bottom());
    assert!(
        (gap - 74.0).abs() < 1.0,
        "74px above the status bar, got {gap}"
    );

    // With a selection the toast stacks above the dock instead of covering it.
    click(vcx, leak(format!("media-card-{}", item.id.0)));
    view.update(vcx, |app, cx| app.show_toast("Second", cx));
    render_ui(vcx);
    let dock = vcx.debug_bounds("multi-select-action-dock").unwrap();
    let toast = vcx.debug_bounds("toast").unwrap();
    assert!(
        toast.bottom() <= dock.top(),
        "toast {toast:?} clears dock {dock:?}"
    );

    // Hovering holds it open past its 8 s, even across a replacement under
    // the resting pointer (no new hover event fires); leaving lets it close.
    vcx.simulate_mouse_move(toast.center(), None, gpui::Modifiers::none());
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_some_and(|t| t.hovered),
            "pointer over the toast"
        )
    });
    view.update(vcx, |app, cx| app.show_toast("Third", cx));
    render_ui(vcx);
    vcx.executor().advance_clock(TOAST_DURATION * 2);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(app.toast.current().is_some(), "a hovered toast stays")
    });
    vcx.simulate_mouse_move(
        gpui::point(gpui::px(5.0), gpui::px(5.0)),
        None,
        gpui::Modifiers::none(),
    );
    vcx.executor().advance_clock(TOAST_DURATION);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.toast.current().is_none(),
            "it closes once the pointer leaves"
        )
    });
}

#[core::prelude::v1::test]
fn error_banner_splits_title_retries_and_esc_dismisses() {
    let (_dir, engine) = seed_library("states_error", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.set_error_with_retry(
            "Could not clear thumbnail cache: disk full",
            |app, cx| app.show_toast("retried", cx),
            cx,
        )
    });
    render_ui(vcx);
    for selector in [
        "error-banner-title",
        "error-banner-detail",
        "error-banner-retry",
        "error-banner-dismiss",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "banner shows {selector}"
        );
    }
    click(vcx, "error-banner-retry");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "Retry clears the banner");
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("retried"),
            "Retry runs the failed action"
        );
    });

    // A visible toast takes Esc first; clear it so Esc reaches the banner.
    view.update(vcx, |app, cx| {
        app.dismiss_toast(cx);
        app.set_error("Plain failure", cx)
    });
    view.update(vcx, |app, _| {
        assert!(app.error_retry.is_none(), "no Retry by default")
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("error-banner-dismiss-text:Dismiss")
            .is_some(),
        "the dismiss control is a text button reading Dismiss"
    );
    let dismiss_h = f32::from(
        vcx.debug_bounds("error-banner-dismiss")
            .expect("dismiss renders")
            .size
            .height,
    );
    assert!(
        dismiss_h < 26.0,
        "Dismiss is a text button, not the old 26 px box, got {dismiss_h}"
    );
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "Esc dismisses")
    });

    view.update(vcx, |app, cx| app.set_error("Another failure", cx));
    render_ui(vcx);
    click(vcx, "error-banner-dismiss");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "clicking Dismiss dismisses")
    });
}

#[core::prelude::v1::test]
fn recently_deleted_counts_notes_and_hides_buttons_when_empty() {
    let (_dir, engine) = seed_library("states_trash", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Empty: no bulk buttons, no "Show all photos", new copy.
    view.update(vcx, |app, cx| app.select_album(AlbumSelection::Trash, cx));
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    for absent in [
        "empty-primary",
        "empty-secondary",
        "header-empty-trash-btn",
        "trash-note",
    ] {
        assert!(
            vcx.debug_bounds(absent).is_none(),
            "empty trash has no {absent}"
        );
    }
    assert!(
        vcx.debug_bounds("empty-state-headline-text:Nothing waiting to be removed")
            .is_some(),
        "empty trash says nothing is waiting to be removed"
    );
    #[cfg(not(target_os = "linux"))]
    let body = "Deleted items stay here until you restore them or remove them for good. \
                Lumina leaves their original files on disk unless you choose to move them to \
                the macOS Trash or delete them.";
    // Linux has no "macOS Trash"; the body names the system's trash instead.
    #[cfg(target_os = "linux")]
    let body = "Deleted items stay here until you restore them or remove them for good. \
                Lumina leaves their original files on disk unless you choose to move them to \
                your system's trash or delete them.";
    assert!(
        vcx.debug_bounds(leak(format!("empty-state-body-text:{body}")))
            .is_some(),
        "empty trash body says how deletion works, without a retention period"
    );

    engine.set_trashed(item.id, true).unwrap();
    view.update(vcx, |app, cx| {
        app.refresh_album_counts();
        app.select_album(AlbumSelection::Trash, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta()
                .as_deref()
                .and_then(|meta| meta.split(" · ").next()),
            Some("1 item"),
            "the meta line counts the trash (the seed's date is its file time)"
        )
    });
    for selector in [
        "page-meta",
        "trash-note",
        "header-empty-trash-btn",
        "header-restore-all-btn",
    ] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "trash with items shows {selector}"
        );
    }

    // Keyboard paths: ⌘K reaches Empty Trash… and Restore All.
    focus_app(vcx, &view);
    keystrokes(vcx, "secondary-k e m p t y space t r a s h enter");
    view.update(vcx, |app, _| {
        assert!(
            app.delete_confirm_state.is_some(),
            "Empty Trash… asks first"
        )
    });
    keystrokes(vcx, "escape");
    keystrokes(vcx, "secondary-k r e s t o r e space a l l enter");
    assert!(
        !engine.get_item_by_id(item.id).unwrap().is_trashed,
        "Restore All puts the item back"
    );
}

#[core::prelude::v1::test]
fn restore_all_reports_what_it_did() {
    let (_dir, engine) = seed_library("states_restore_all", 2);
    let item = engine.get_item_slice(0..1)[0].clone();
    engine.set_trashed(item.id, true).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| app.restore_all_trash(cx));
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none());
        assert_eq!(
            app.toast.current().map(|t| t.message.as_str()),
            Some("Restored 1 item(s)"),
            "Restore All confirms instead of staying silent"
        );
    });
    assert!(!engine.get_item_by_id(item.id).unwrap().is_trashed);
}

fn favorite_rule() -> SmartAlbumRule {
    SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }
}

#[core::prelude::v1::test]
fn empty_smart_album_shows_rule_chips_edit_and_delete() {
    use crate::app::rule_label::rule_label;

    let (_dir, engine) = seed_library("states_album", 2);
    let rules = [
        favorite_rule(),
        SmartAlbumRule {
            field: RuleField::Rating,
            operator: RuleOperator::GreaterThan,
            value: RuleValue::Integer(4),
        },
    ];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state-headline-text:Nothing matches these rules yet")
            .is_some(),
        "the headline speaks of the rules, not the title"
    );
    assert!(
        vcx.debug_bounds(
            "empty-state-body-text:This album fills itself. As soon as a photo matches, it appears here."
        )
        .is_some(),
        "the body says the album fills itself"
    );
    for (i, rule) in rules.iter().enumerate() {
        assert!(
            vcx.debug_bounds(leak(format!("empty-rule-chip-{i}")))
                .is_some(),
            "one chip per rule: chip {i} is missing"
        );
        let text = format!("empty-rule-chip-{i}-text:{}", rule_label(rule));
        assert!(
            vcx.debug_bounds(leak(text.clone())).is_some(),
            "chip {i} reads its rule label, missing {text}"
        );
    }
    assert!(
        vcx.debug_bounds(leak(format!("empty-rule-chip-{}", rules.len())))
            .is_none(),
        "no chip beyond the album's rules"
    );
    assert!(
        vcx.debug_bounds("empty-primary-text:Edit rules").is_some(),
        "the primary action is Edit rules"
    );
    assert!(
        vcx.debug_bounds("empty-secondary-text:Delete album")
            .is_some(),
        "the secondary action is Delete album"
    );

    click(vcx, "empty-secondary");
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("smart-album-delete-dialog").is_some(),
        "Delete album draws the delete confirmation"
    );
    view.update(vcx, |app, cx| {
        assert_eq!(
            app.album_draft.confirm_delete,
            Some(album.id),
            "Delete album asks for confirmation first"
        );
        app.cancel_delete_smart_album(cx);
    });
    assert!(
        engine.list_smart_albums().iter().any(|a| a.id == album.id),
        "Delete album never deletes without the confirmation"
    );
    render_ui(vcx);
    click(vcx, "empty-primary");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Edit rules opens the builder")
    });
}

#[core::prelude::v1::test]
fn empty_library_shows_its_path() {
    let (_empty_dir, empty) = seed_library("states_empty_lib", 0);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, empty);
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = false;
        }
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-library-path").is_some(),
        "an empty library shows its path on its own mono line"
    );
}

/// Three clips (1 h + 2 m + 1 m, 1,000,000 bytes each) in June 2025 beside
/// the seed's photo, as rows only: no file, so no watcher rescan resets them.
fn library_with_clips(
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
    std::sync::Arc<lumina_core::LuminaEngine>,
    Vec<lumina_core::MediaId>,
) {
    let (dir, engine) = seed_library(prefix, 1);
    let tpl = engine.get_item_slice(0..1)[0].clone();
    let clips: Vec<_> = [3_600_000, 120_000, 60_000]
        .into_iter()
        .enumerate()
        .map(|(i, duration)| {
            let mut clip = tpl.clone();
            clip.file_name = format!("clip_{i}.mov");
            clip.path = dir.join(&clip.file_name);
            clip.media_type = lumina_core::MediaType::Video;
            clip.duration_ms = Some(duration);
            clip.file_size = 1_000_000;
            (clip.year, clip.month) = (2025, 6);
            clip
        })
        .collect();
    let stored = engine.catalog().insert_or_update_batch(&clips).unwrap();
    (dir, engine, stored.iter().map(|clip| clip.id).collect())
}

#[core::prelude::v1::test]
fn videos_header_reads_clips() {
    let (_dir, engine, _clips) = library_with_clips("states_videos_meta");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::System(lumina_core::SystemAlbum::Videos), cx)
    });
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("3 clips · 1 h 3 m · 2.9 MB"),
            "Videos counts clips and sums their duration and size, not the photo's"
        );
        assert_eq!(
            app.month_header_count_label().as_deref(),
            Some("3 clips"),
            "the month band counts the view's clips in that month, as clips"
        );
        assert_eq!(
            app.mosaic_day_header(20250601, 1).count,
            "1 clip",
            "a Mosaic day header in Videos counts clips too"
        );
        let select = app.mosaic_day_header(20250601, 2).select_label;
        assert!(
            select.starts_with("Select 2 clips from "),
            "the day's Select label names clips in Videos: {select}"
        );
        let select = app.mosaic_day_header(20250601, 1).select_label;
        assert!(
            select.starts_with("Select 1 clip from "),
            "and the singular too: {select}"
        );
    });
    assert!(
        vcx.debug_bounds("page-meta").is_some(),
        "the meta line is painted"
    );

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::AllLibrary, cx)
    });
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert!(
            app.page_meta()
                .is_some_and(|meta| meta.starts_with("4 items")),
            "other views keep counting items: {:?}",
            app.page_meta()
        );
        assert_eq!(app.mosaic_day_header(20250601, 2).count, "2 items");
    });
}

#[core::prelude::v1::test]
fn videos_meta_updates_after_a_clip_is_trashed() {
    let (_dir, engine, clips) = library_with_clips("states_videos_trash");
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::System(lumina_core::SystemAlbum::Videos), cx)
    });
    render_ui(vcx);
    engine.batch_set_trashed(&clips[..1], true).unwrap();
    render_ui(vcx);
    render_ui(vcx);
    view.update(vcx, |app, _| {
        assert_eq!(
            app.page_meta().as_deref(),
            Some("2 clips · 3 m · 1.9 MB"),
            "a trashed clip leaves the totals at the next revision; under an hour reads minutes only"
        );
    });
}

#[core::prelude::v1::test]
fn empty_album_offers_add_from_library() {
    use crate::app::command_palette::CommandId;

    let (_dir, engine) = seed_library("states_empty_album_add", 1);
    let album = engine.create_album_with("Iceland 2024", &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let open_album = |vcx: &mut gpui::VisualTestContext| {
        view.update(vcx, |app, cx| {
            app.select_album(AlbumSelection::Album(album.id), cx)
        });
        render_ui(vcx);
    };
    let assert_went_to_library = |vcx: &mut gpui::VisualTestContext, path: &str| {
        view.update(vcx, |app, cx| {
            assert_eq!(
                app.selection,
                AlbumSelection::AllLibrary,
                "{path} goes to All Media"
            );
            let toast = app.toast.current().map(|t| t.message.clone());
            assert!(
                toast.as_deref().is_some_and(|m| m.contains("Add to Album")),
                "{path} points at Add to Album, got {toast:?}"
            );
            app.dismiss_toast(cx);
        });
    };

    open_album(vcx);
    assert!(
        vcx.debug_bounds(
            "empty-state-body-text:Items stay in your library \u{2014} an album is just a view of them."
        )
        .is_some(),
        "the empty album says items stay in the library"
    );
    assert!(
        vcx.debug_bounds("empty-primary-text:Add from Library")
            .is_some(),
        "the empty album's primary action is Add from Library"
    );
    view.update(vcx, |app, _| {
        assert!(
            app.command_applies(CommandId::AddFromLibrary),
            "⌘K offers Add from Library on an empty album"
        )
    });

    click(vcx, "empty-primary");
    assert_went_to_library(vcx, "the button");
    view.update(vcx, |app, _| {
        assert!(
            !app.command_applies(CommandId::AddFromLibrary),
            "no Add from Library outside an empty album"
        )
    });

    open_album(vcx);
    focus_app(vcx, &view);
    keystrokes(
        vcx,
        "secondary-k a d d space f r o m space l i b r a r y enter",
    );
    render_ui(vcx);
    assert_went_to_library(vcx, "⌘K Add from Library");

    // The album's title lookup fails (it left the list): the button and
    // ⌘K agree that there is nothing to add to.
    open_album(vcx);
    view.update(vcx, |app, cx| {
        app.albums.list.retain(|a| a.id != album.id);
        cx.notify();
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state-headline-text:This album is empty")
            .is_some(),
        "the empty album still shows, without its title"
    );
    let button = vcx
        .debug_bounds("empty-primary-text:Add from Library")
        .is_some();
    let command = view.update(vcx, |app, _| app.command_applies(CommandId::AddFromLibrary));
    assert!(
        !button && !command,
        "button ({button}) and ⌘K ({command}) share one condition"
    );
}

#[core::prelude::v1::test]
fn empty_album_long_title_wraps_and_keeps_the_button_in_view() {
    let (_dir, engine) = seed_library("states_empty_album_long", 1);
    let title = "A very long album title for the Iceland road trip of summer 2024 and more";
    assert!(title.chars().count() >= 60);
    let album = engine.create_album_with(title, &[]).unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(900.0), gpui::px(600.0)));
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    render_ui(vcx);

    let headline = vcx
        .debug_bounds("empty-state-headline")
        .expect("the empty album draws a headline");
    assert!(
        f32::from(headline.size.width) <= 430.0,
        "the headline wraps within 430 px, got {:?}",
        headline.size.width
    );
    assert!(
        f32::from(headline.size.height) > 30.0,
        "a 60+ character headline wraps onto a second line, got {:?}",
        headline.size.height
    );
    let window = vcx.update(|window, _| window.viewport_size());
    let primary = vcx
        .debug_bounds("empty-primary")
        .expect("the empty album keeps its primary action");
    assert!(
        primary.left() >= gpui::px(0.0)
            && primary.top() >= gpui::px(0.0)
            && primary.right() <= window.width
            && primary.bottom() <= window.height,
        "Add from Library stays inside the {window:?} window, got {primary:?}"
    );
}

/// Ten terms read as a word; more than ten switch to digits.
#[core::prelude::v1::test]
fn empty_search_headline_uses_digits_past_ten_terms() {
    let (_dir, engine) = seed_library("states_digits", 2);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);

    for (n, headline) in [
        (10, "Nothing matches all ten filters"),
        (11, "Nothing matches all 11 filters"),
        (14, "Nothing matches all 14 filters"),
    ] {
        let query: Vec<String> = (0..n).map(|i| format!("zzqx{i}")).collect();
        let query = query.join(" ");
        view.update(vcx, |app, cx| app.set_search_query(&query, cx));
        render_ui(vcx);
        assert!(
            vcx.debug_bounds(leak(format!("empty-state-headline-text:{headline}")))
                .is_some(),
            "{n} terms must read {headline:?}: words up to ten, digits above"
        );
        assert!(
            vcx.debug_bounds(leak(format!("empty-search-term-{}", n - 1)))
                .is_some()
                && vcx
                    .debug_bounds(leak(format!("empty-search-term-{n}")))
                    .is_none(),
            "{n} terms must draw exactly {n} Drop chips"
        );
    }
}

/// With the sidebar and the inspector both open, many rule chips wrap inside
/// the middle column and overflow neither it nor the window.
#[core::prelude::v1::test]
fn empty_smart_album_chips_wrap_between_sidebar_and_inspector() {
    let (_dir, engine) = seed_library("states_chips", 1);
    let rules: Vec<_> = (0..9)
        .map(|i| SmartAlbumRule {
            field: RuleField::Camera,
            operator: RuleOperator::Contains,
            value: RuleValue::Text(format!("Camera model {i}")),
        })
        .collect();
    let album = engine
        .create_smart_album("Many rules", "folder", MatchMode::Any, &rules)
        .unwrap();
    // 960×600 is the window's minimum (main.rs); 900 is narrower still.
    for width in [900.0, 960.0] {
        let mut cx = crate::tests::TestApp::single();
        let (view, vcx) = app_with_library(&mut cx, engine.clone());
        vcx.simulate_resize(gpui::size(gpui::px(width), gpui::px(600.0)));
        view.update(vcx, |app, cx| {
            app.sidebar_collapsed = false;
            app.inspector_open = true;
            app.select_album(AlbumSelection::User(album.id), cx)
        });
        render_ui(vcx);

        let sidebar = vcx.debug_bounds("sidebar").expect("the sidebar is open");
        let inspector = vcx
            .debug_bounds("inspector-panel")
            .expect("the inspector is open");
        let column = vcx
            .debug_bounds("empty-state")
            .expect("the empty state renders");
        assert!(
            column.left() >= sidebar.right() && column.right() <= inspector.left(),
            "at {width} wide the empty state must shrink to the column between the panels, \
             sidebar {sidebar:?}, inspector {inspector:?}, got {column:?}"
        );
        let chips: Vec<_> = (0..rules.len())
            .map(|i| {
                vcx.debug_bounds(leak(format!("empty-rule-chip-{i}")))
                    .unwrap_or_else(|| panic!("chip {i} is drawn"))
            })
            .collect();
        let first_top = chips[0].top();
        assert!(
            chips.iter().any(|chip| chip.top() > first_top),
            "at {width} wide nine chips wrap onto more than one row"
        );
        let window = vcx.update(|window, _| window.viewport_size());
        for (i, chip) in chips.iter().enumerate() {
            assert!(
                chip.left() >= column.left()
                    && chip.right() <= column.right()
                    && chip.top() >= column.top()
                    && chip.bottom() <= column.bottom()
                    && chip.bottom() <= window.height,
                "at {width} wide chip {i} must stay inside the column {column:?} and the window, \
                 got {chip:?}"
            );
        }
        for button in ["empty-primary", "empty-secondary"] {
            let b = vcx
                .debug_bounds(button)
                .unwrap_or_else(|| panic!("{button} renders"));
            assert!(
                b.left() >= column.left() && b.right() <= column.right(),
                "at {width} wide {button} must stay inside the column {column:?}, got {b:?}"
            );
        }
    }
}

/// An empty library mid-scan also draws at the Hero size.
#[core::prelude::v1::test]
fn empty_library_scanning_uses_hero_size() {
    let (_dir, engine) = seed_library("states_scan_hero", 0);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, _| {
        if let Some(state) = app.grid_state.as_mut() {
            state.is_scanning = true;
        }
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-state-headline-text:Scanning…")
            .is_some(),
        "precondition: the scanning copy is shown"
    );
    let glyph = vcx.debug_bounds("empty-state-glyph").expect("a glyph");
    assert_eq!(
        (f32::from(glyph.size.width), f32::from(glyph.size.height)),
        (150.0, 150.0),
        "an empty library mid-scan must use the 150 px Hero glyph"
    );
    let primary = vcx
        .debug_bounds("empty-primary")
        .expect("Choose Another Folder… stays");
    assert_eq!(
        f32::from(primary.size.height),
        40.0,
        "the Hero button is 40 high"
    );
}

/// A long title and subtitle truncate before Undo, which
/// keeps its full width inside the card.
#[core::prelude::v1::test]
fn long_toast_text_truncates_before_the_action() {
    let (_dir, engine) = seed_library("states_toast_long", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    vcx.simulate_resize(gpui::size(gpui::px(1280.0), gpui::px(800.0)));

    view.update(vcx, |app, cx| app.show_undo_toast("Rated 1 item", cx));
    render_ui(vcx);
    let natural = vcx
        .debug_bounds("toast-action")
        .expect("Undo renders")
        .size
        .width;

    let long = "Moved 1,234 photos and videos from the Summer Holiday in the Mountains \
                album to Recently Deleted, including every edited copy and sidecar";
    view.update(vcx, |app, cx| {
        app.show_toast_kind(
            ToastKind::Positive,
            long,
            Some(long),
            Some(ToastAction::Undo),
            cx,
        )
    });
    render_ui(vcx);
    let toast = vcx.debug_bounds("toast").expect("the toast renders");
    let action = vcx.debug_bounds("toast-action").expect("Undo renders");
    let dismiss = vcx.debug_bounds("toast-dismiss").expect("× renders");
    for line in ["toast-message", "toast-subtitle"] {
        let b = vcx
            .debug_bounds(line)
            .unwrap_or_else(|| panic!("{line} renders"));
        assert!(
            b.right() <= action.left(),
            "{line} must end before Undo, {b:?} vs {action:?}"
        );
        assert!(
            f32::from(b.size.height) <= 20.0,
            "{line} must stay on one line, got {b:?}"
        );
    }
    assert_eq!(
        action.size.width, natural,
        "Undo must keep its full width beside long text"
    );
    assert!(
        action.left() >= toast.left()
            && action.right() <= dismiss.left()
            && dismiss.right() <= toast.right(),
        "Undo and × must stay inside the card, toast {toast:?}, Undo {action:?}, × {dismiss:?}"
    );
}

/// Which empty-state button draws the focus ring, by its test marker.
fn empty_focus(vcx: &mut gpui::VisualTestContext) -> (bool, bool) {
    (
        vcx.debug_bounds("empty-primary-focused").is_some(),
        vcx.debug_bounds("empty-secondary-focused").is_some(),
    )
}

/// An empty tag view: Show all photos (primary) and Manage tags (secondary).
fn empty_tag_view<'a>(
    prefix: &str,
    cx: &'a mut gpui::TestAppContext,
) -> (
    TempLibrary,
    gpui::Entity<crate::LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, engine) = seed_library(prefix, 1);
    let (view, vcx) = app_with_library(cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this".into()), cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-secondary").is_some(),
        "precondition: the empty tag view shows two buttons"
    );
    (dir, view, vcx)
}

/// AGENTS §11: Tab / ⇧Tab move a visible ring between the shown buttons,
/// Esc clears it and ↵ presses the focused one, and only that.
#[core::prelude::v1::test]
fn empty_state_buttons_take_tab_and_enter() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_tab", &mut cx);
    assert_eq!(empty_focus(vcx), (false, false), "no button starts focused");
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (true, false),
        "Tab focuses the first button"
    );
    keystrokes(vcx, "tab");
    assert_eq!(empty_focus(vcx), (false, true), "Tab moves to the second");
    keystrokes(vcx, "tab");
    assert_eq!(empty_focus(vcx), (true, false), "Tab wraps to the first");
    keystrokes(vcx, "shift-tab");
    assert_eq!(empty_focus(vcx), (false, true), "⇧Tab goes back, wrapping");
    keystrokes(vcx, "escape");
    assert_eq!(empty_focus(vcx), (false, false), "Esc clears the focus");

    keystrokes(vcx, "tab enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::AllLibrary,
            "↵ on the focused Show all photos shows All Media"
        );
        // The grid's ↵ with nothing selected selects the first item and
        // opens it (All Media holds one), so either would show here.
        assert!(
            !app.is_fullscreen_viewer && app.selected_item.is_none(),
            "↵ presses the button only; the grid's ↵ must not also select and open an item"
        );
    });

    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this".into()), cx)
    });
    render_ui(vcx);
    keystrokes(vcx, "shift-tab enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Tags,
            "⇧Tab ↵ presses the last button, Manage tags"
        )
    });
}

/// With one button Tab focuses it, and ↵ presses it.
#[core::prelude::v1::test]
fn empty_search_single_button_takes_tab_and_enter() {
    let (_dir, engine) = seed_library("states_tab_one", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| app.set_search_query("zzqxnothing", cx));
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-primary").is_none()
            && vcx.debug_bounds("empty-secondary").is_some(),
        "precondition: a search with no results shows only Clear all filters"
    );
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "Tab focuses the only button"
    );
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "Tab stays on the only button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert!(
            app.search_query.is_empty(),
            "↵ on Clear all filters clears the search"
        )
    });
}

/// A focus set in one empty state must not press a button in the next.
#[core::prelude::v1::test]
fn empty_state_focus_resets_when_the_view_changes() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_tab_reset", &mut cx);
    keystrokes(vcx, "tab tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Manage tags focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Tag("nothing-has-this-either".into()), cx)
    });
    render_ui(vcx);
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "another empty view starts with no focused button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.selection,
            AlbumSelection::Tag("nothing-has-this-either".into()),
            "↵ with no focused button must not press the stale Manage tags"
        )
    });
}

/// Leaving an empty view and coming back (a click or Back) starts with no
/// focused button, as docs/accessibility.md promises.
#[core::prelude::v1::test]
fn empty_state_focus_does_not_return_with_the_view() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = empty_tag_view("states_tab_return", &mut cx);
    let tag = AlbumSelection::Tag("nothing-has-this".into());
    keystrokes(vcx, "tab tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Manage tags focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
        app.select_album(tag.clone(), cx);
    });
    render_ui(vcx);
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by a click starts with no focused button"
    );
    keystrokes(vcx, "enter");
    view.update(vcx, |app, _| {
        assert_eq!(app.selection, tag, "↵ must not press the stale Manage tags")
    });

    // Back restores a search (a tag no item has falls back to All Media).
    view.update(vcx, |app, cx| app.set_search_query("zzqxnothing", cx));
    render_ui(vcx);
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Clear all filters focused"
    );
    view.update(vcx, |app, cx| {
        app.select_album(
            AlbumSelection::System(lumina_core::SystemAlbum::Favorites),
            cx,
        );
    });
    render_ui(vcx);
    keystrokes(vcx, "secondary-[");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "zzqxnothing",
            "precondition: Back returns to the search"
        )
    });
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by Back starts with no focused button"
    );

    // Back is a way out too: Forward then returns with no focus.
    keystrokes(vcx, "tab");
    assert_eq!(
        empty_focus(vcx),
        (false, true),
        "precondition: Clear all filters focused"
    );
    keystrokes(vcx, "secondary-[");
    keystrokes(vcx, "secondary-]");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.search_query, "zzqxnothing",
            "precondition: Forward returns to the search"
        )
    });
    assert_eq!(
        empty_focus(vcx),
        (false, false),
        "returning to the view by Forward starts with no focused button"
    );
}
