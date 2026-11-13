//! v3 toasts, error banner and empty states (FIXES §14).

use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::views::sidebar::AlbumSelection;
use crate::views::toast::TOAST_DURATION;

#[core::prelude::v1::test]
fn toast_sits_centred_over_the_grid_and_waits_while_hovered() {
    let (dir, engine) = seed_library("states_toast", 3);
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
    let dismiss = vcx.debug_bounds("toast-dismiss").unwrap();
    assert_eq!(f32::from(dismiss.size.width), 26.0, "26px dismiss button");

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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn error_banner_splits_title_retries_and_esc_dismisses() {
    let (dir, engine) = seed_library("states_error", 2);
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
    focus_app(vcx, &view);
    keystrokes(vcx, "escape");
    view.update(vcx, |app, _| {
        assert!(app.error_message.is_none(), "Esc dismisses")
    });

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn recently_deleted_counts_notes_and_hides_buttons_when_empty() {
    let (dir, engine) = seed_library("states_trash", 3);
    let item = engine.get_item_slice(0..1)[0].clone();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine.clone());

    // Empty: no bulk buttons, no "Show all photos", new copy.
    view.update(vcx, |app, cx| app.select_album(AlbumSelection::Trash, cx));
    render_ui(vcx);
    assert!(vcx.debug_bounds("empty-state").is_some());
    for absent in ["empty-primary", "header-empty-trash-btn", "trash-note"] {
        assert!(
            vcx.debug_bounds(absent).is_none(),
            "empty trash has no {absent}"
        );
    }

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

    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn restore_all_reports_what_it_did() {
    let (dir, engine) = seed_library("states_restore_all", 2);
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
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn empty_smart_album_offers_only_edit_rules_and_library_path_is_mono() {
    let (dir, engine) = seed_library("states_album", 2);
    let rules = [SmartAlbumRule {
        field: RuleField::IsFavorite,
        operator: RuleOperator::Equals,
        value: RuleValue::Bool(true),
    }];
    let album = engine
        .create_smart_album("Keepers", "folder", MatchMode::All, &rules)
        .unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::User(album.id), cx)
    });
    render_ui(vcx);
    assert!(
        vcx.debug_bounds("empty-secondary").is_none(),
        "one button only"
    );
    click(vcx, "empty-primary");
    view.update(vcx, |app, _| {
        assert!(app.album_draft.modal_open, "Edit Rules… opens the builder")
    });
    let _ = std::fs::remove_dir_all(&dir);

    let (empty_dir, empty) = seed_library("states_empty_lib", 0);
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
    let _ = std::fs::remove_dir_all(&empty_dir);
}

/// Three clips (1 h + 2 m + 1 m, 1,000,000 bytes each) in June 2025 beside
/// the seed's photo, as rows only: no file, so no watcher rescan resets them.
fn library_with_clips(
    prefix: &str,
) -> (
    std::path::PathBuf,
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
    let (dir, engine, _clips) = library_with_clips("states_videos_meta");
    let _cleanup = super::downloads::TempLibrary(dir);
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
    let (dir, engine, clips) = library_with_clips("states_videos_trash");
    let _cleanup = super::downloads::TempLibrary(dir);
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

/// Hosts the empty-state shell at the `Hero` size, which no view uses yet.
struct HeroHost(gpui::Entity<crate::app::LuminaApp>);

impl gpui::Render for HeroHost {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        self.0.update(cx, |app, cx| {
            app.render_empty_state(crate::app::render::EmptySize::Hero, cx)
        })
    }
}

#[core::prelude::v1::test]
fn empty_state_shell_matches_design() {
    let (dir, engine) = seed_library("states_empty_shell", 1);
    let album = engine.create_album("Nothing yet").unwrap();
    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    view.update(vcx, |app, cx| {
        app.select_album(AlbumSelection::Album(album.id), cx)
    });
    render_ui(vcx);

    let glyph = vcx
        .debug_bounds("empty-state-glyph")
        .expect("the empty album draws the glyph");
    let side = (f32::from(glyph.size.width), f32::from(glyph.size.height));
    assert_eq!(side, (54.0, 54.0), "the glyph is a 54 px circle");
    let headline = vcx
        .debug_bounds("empty-state-headline")
        .expect("the empty album draws a headline");
    assert!(
        vcx.debug_bounds("empty-state-body").is_some(),
        "the empty album draws a body line"
    );
    let gap = f32::from(headline.top() - glyph.bottom());
    assert!(
        (gap - 14.0).abs() <= 1.0,
        "14 px from the glyph to the headline, got {gap}"
    );
    let primary = vcx.debug_bounds("empty-primary").expect("a primary action");
    assert_eq!(
        f32::from(primary.size.height),
        34.0,
        "the primary button is 34 px tall"
    );
    let _ = std::fs::remove_dir_all(&dir);

    // Hero (First Run): no library, so the shell offers Open Library….
    use gpui::AppContext as _;
    let mut cx = crate::tests::TestApp::single();
    let (_host, vcx) =
        cx.add_window_view(|_window, cx| HeroHost(cx.new(crate::app::LuminaApp::new)));
    render_ui(vcx);
    let glyph = vcx
        .debug_bounds("empty-state-glyph")
        .expect("the hero shell draws the glyph");
    let side = (f32::from(glyph.size.width), f32::from(glyph.size.height));
    assert_eq!(side, (150.0, 150.0), "the hero glyph is 150 px");
    let primary = vcx
        .debug_bounds("empty-primary")
        .expect("the hero shell has a primary action");
    assert_eq!(
        f32::from(primary.size.height),
        40.0,
        "the hero primary button is 40 px tall"
    );
}
