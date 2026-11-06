//! Viewer zoom presets: ⌘0 Fit, ⌘1 100%, ⌘2 200%, ⌘3 Fill, ⌘4 400%, the
//! navigator preset buttons and the palette zoom commands. Plain digits no
//! longer zoom (plan 5 gives them to ratings).

use crate::tests::ui_automation::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::tests::viewer_and_video::{render_viewer_ui, viewer_with_gradient};
use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MIN_SCALE};
use crate::*;

/// A photo viewer whose item is a 6000x4500 original, so 100% is above Fit
/// and every preset gives a distinct scale.
fn large_photo_viewer<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
) -> (
    std::path::PathBuf,
    gpui::Entity<LuminaApp>,
    &'a mut gpui::VisualTestContext,
) {
    let (dir, view, vcx) = viewer_with_gradient(cx, prefix, 1440.0, 900.0);
    view.update(vcx, |app, cx| {
        let item = app.selected_item.as_mut().expect("viewer has an item");
        item.width = 6000;
        item.height = 4500;
        cx.notify();
    });
    render_viewer_ui(vcx);
    focus_app(vcx, &view);
    (dir, view, vcx)
}

fn expected(app: &LuminaApp, preset: ViewerZoomPreset) -> f32 {
    preset.scale(app.viewer_state.canvas)
}

#[core::prelude::v1::test]
fn cmd_digit_zoom_presets_in_viewer() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, view, vcx) = large_photo_viewer(&mut cx, "cmd_digit_presets");
    let actual = view.update(vcx, |app, _| expected(app, ViewerZoomPreset::Actual));
    assert!(
        actual > 1.05,
        "fixture: 100% must be above Fit, got {actual}"
    );

    let cases = [
        ("secondary-1", ViewerZoomPreset::Actual, "100%"),
        ("secondary-2", ViewerZoomPreset::TwoX, "200%"),
        ("secondary-4", ViewerZoomPreset::FourX, "400%"),
        ("secondary-3", ViewerZoomPreset::Fill, ""),
        ("secondary-0", ViewerZoomPreset::Fit, "Fit"),
    ];
    for (keys, preset, label) in cases {
        keystrokes(vcx, keys);
        view.update(vcx, |app, _| {
            let want = expected(app, preset);
            assert!(
                (app.viewer_state.zoom_scale - want).abs() < 1e-4,
                "{keys} must apply {preset:?} ({want}), got {}",
                app.viewer_state.zoom_scale
            );
            let want_label = if label.is_empty() {
                format!("{}%", (want / actual * 100.0).round() as i32)
            } else {
                label.to_string()
            };
            assert_eq!(app.viewer_state.zoom_label(), want_label, "{keys} label");
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn plain_digits_do_not_zoom_viewer() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, view, vcx) = large_photo_viewer(&mut cx, "plain_digits");
    for key in ["1", "2", "3", "4", "0"] {
        keystrokes(vcx, key);
        view.update(vcx, |app, _| {
            assert_eq!(
                app.viewer_state.zoom_scale, ZOOM_MIN_SCALE,
                "plain {key} must not zoom (digits are reserved for ratings)"
            );
            assert!(
                app.is_fullscreen_viewer,
                "plain {key} must not close the viewer"
            );
        });
    }
    // Zoomed in, plain 0 must not reset to Fit either.
    keystrokes(vcx, "secondary-2");
    let zoomed = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    assert!(zoomed > ZOOM_MIN_SCALE);
    keystrokes(vcx, "0");
    view.update(vcx, |app, _| {
        assert_eq!(app.viewer_state.zoom_scale, zoomed, "plain 0 must not fit");
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn cmd_digits_do_nothing_in_the_grid() {
    let (dir, engine) = seed_library("cmd_digits_grid", 4);
    let mut cx = gpui::TestAppContext::single();
    let (view, vcx) = app_with_library(&mut cx, engine);
    let card = leak(format!(
        "media-card-{}",
        view.update(vcx, |app, _| app.get_active_slice(0..1)[0].id.0)
    ));
    click(vcx, card);
    let before = view.update(vcx, |app, _| {
        (
            app.grid_columns,
            app.selected_index,
            app.selection.clone(),
            app.sort.field_key().to_string(),
            app.viewer_state.zoom_scale,
        )
    });
    keystrokes(
        vcx,
        "secondary-0 secondary-1 secondary-2 secondary-3 secondary-4",
    );
    view.update(vcx, |app, _| {
        assert!(
            !app.is_fullscreen_viewer,
            "⌘digits must not open the viewer"
        );
        let after = (
            app.grid_columns,
            app.selected_index,
            app.selection.clone(),
            app.sort.field_key().to_string(),
            app.viewer_state.zoom_scale,
        );
        assert_eq!(
            after, before,
            "⌘digits are viewer-only and do nothing in the grid"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn navigator_preset_buttons_apply_presets() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, view, vcx) = large_photo_viewer(&mut cx, "navigator_presets");
    // The navigator renders only while zoomed.
    keystrokes(vcx, "secondary-2");
    for (selector, preset) in [
        ("viewer-zoom-preset-fill", ViewerZoomPreset::Fill),
        ("viewer-zoom-preset-actual", ViewerZoomPreset::Actual),
        ("viewer-zoom-preset-fit", ViewerZoomPreset::Fit),
    ] {
        click(vcx, selector);
        render_viewer_ui(vcx);
        view.update(vcx, |app, _| {
            let want = expected(app, preset);
            assert!(
                (app.viewer_state.zoom_scale - want).abs() < 1e-4,
                "{selector} must apply {preset:?} ({want}), got {}",
                app.viewer_state.zoom_scale
            );
        });
    }
    assert!(
        vcx.debug_bounds("viewer-navigator").is_none(),
        "at Fit the navigator and its presets hide"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn zoom_rows(view: &gpui::Entity<LuminaApp>, vcx: &mut gpui::VisualTestContext) -> Vec<String> {
    view.update(vcx, |app, _| {
        app.command_query = "zoom".into();
        let rows = app
            .palette_rows()
            .into_iter()
            .filter(|r| !r.is_go_to())
            .map(|r| r.selector)
            .filter(|s| !s.contains("zoom_in") && !s.contains("zoom_out"))
            .collect();
        app.command_query.clear();
        rows
    })
}

#[core::prelude::v1::test]
fn palette_zoom_commands_only_in_photo_viewer() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, view, vcx) = large_photo_viewer(&mut cx, "palette_zoom");

    let shown = zoom_rows(&view, vcx);
    for key in [
        "command.zoom_fit",
        "command.zoom_fill",
        "command.zoom_actual",
        "command.zoom_200",
        "command.zoom_400",
    ] {
        assert!(
            shown.contains(&format!("command-item-{key}")),
            "{key} must be offered in the photo viewer: {shown:?}"
        );
    }

    keystrokes(vcx, "secondary-k z o o m space t o space f i l l enter");
    view.update(vcx, |app, _| {
        assert!(!app.command_palette_open);
        let want = expected(app, ViewerZoomPreset::Fill);
        assert!(want > ZOOM_MIN_SCALE, "fixture: Fill must be above Fit");
        assert!(
            (app.viewer_state.zoom_scale - want).abs() < 1e-4,
            "\"Zoom to Fill\" must apply Fill ({want}), got {}",
            app.viewer_state.zoom_scale
        );
    });

    // A video in the viewer: no zoom commands.
    view.update(vcx, |app, cx| {
        app.selected_item.as_mut().expect("item").media_type = lumina_core::MediaType::Video;
        cx.notify();
    });
    assert!(
        zoom_rows(&view, vcx).is_empty(),
        "video does not zoom, so no zoom commands"
    );

    // The grid: no zoom commands.
    view.update(vcx, |app, cx| {
        app.selected_item.as_mut().expect("item").media_type = lumina_core::MediaType::Image;
        app.close_viewer(cx);
    });
    render_ui(vcx);
    assert!(
        zoom_rows(&view, vcx).is_empty(),
        "viewer zoom commands are not offered in the grid"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[core::prelude::v1::test]
fn ctrl_digits_do_not_zoom_viewer() {
    let mut cx = gpui::TestAppContext::single();
    let (dir, view, vcx) = large_photo_viewer(&mut cx, "ctrl_digit_presets");
    let before = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
    keystrokes(vcx, "ctrl-1 ctrl-2 ctrl-cmd-4");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, before,
            "⌃digits are macOS Switch to Desktop and ⌃⌘ chords belong to the window"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}
