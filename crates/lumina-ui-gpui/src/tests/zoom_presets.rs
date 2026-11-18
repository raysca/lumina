//! Viewer zoom presets: ⌘0 Fit, ⌘1 100%, ⌘2 200%, ⌘3 Fill, ⌘4 400%, the
//! navigator preset buttons and the palette zoom commands. Plain digits no
//! longer zoom (they set ratings).

use crate::tests::helpers::*;
use crate::tests::helpers::{
    app_with_library, click, focus_app, keystrokes, leak, render_ui, seed_library,
};
use crate::tests::helpers::{render_viewer_ui, viewer_with_gradient};
use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MIN_SCALE};
use crate::*;

/// A photo viewer whose item is a 6000x4500 original, so 100% is above Fit
/// and every preset gives a distinct scale.
fn large_photo_viewer<'a>(
    cx: &'a mut gpui::TestAppContext,
    prefix: &str,
) -> (
    crate::tests::helpers::TempLibrary,
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
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = large_photo_viewer(&mut cx, "cmd_digit_presets");
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
}

#[core::prelude::v1::test]
fn plain_digits_do_not_zoom_viewer() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = large_photo_viewer(&mut cx, "plain_digits");
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
}

#[core::prelude::v1::test]
fn cmd_digits_do_nothing_in_the_grid() {
    let (_dir, engine) = seed_library("cmd_digits_grid", 4);
    let mut cx = crate::tests::TestApp::single();
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
}

#[core::prelude::v1::test]
fn navigator_preset_buttons_apply_presets() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = large_photo_viewer(&mut cx, "navigator_presets");
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
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = large_photo_viewer(&mut cx, "palette_zoom");

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
}

#[core::prelude::v1::test]
fn platform_modifier_digits_follow_shortcut_semantics() {
    let mut cx = crate::tests::TestApp::single();
    let (_dir, view, vcx) = large_photo_viewer(&mut cx, "ctrl_digit_presets");
    #[cfg(target_os = "macos")]
    {
        let before = view.update(vcx, |app, _| app.viewer_state.zoom_scale);
        keystrokes(vcx, "ctrl-1 ctrl-2 ctrl-cmd-4");
        view.update(vcx, |app, _| {
            assert_eq!(
                app.viewer_state.zoom_scale, before,
                "⌃digits are macOS Switch to Desktop and ⌃⌘ chords belong to the window"
            );
        });
        // Regression: the Linux port's shared check also refused Fn.
        keystrokes(vcx, "fn-cmd-2");
        view.update(vcx, |app, _| {
            assert_eq!(
                app.viewer_state.zoom_label(),
                "200%",
                "fn+⌘2 still zooms on macOS (laptop keyboards)"
            );
        });
    }
    #[cfg(target_os = "linux")]
    {
        keystrokes(vcx, "ctrl-2");
        view.update(vcx, |app, _| {
            assert_eq!(
                app.viewer_state.zoom_label(),
                "200%",
                "Control is the primary shortcut modifier on Linux"
            );
        });
    }
}

#[core::prelude::v1::test]
fn test_viewer_zoom_presets_hud_label_and_shortcuts() {
    use crate::views::viewer_state::{ViewerZoomPreset, ZOOM_MAX_SCALE, ZOOM_MIN_SCALE};

    // Keys reach the viewer only with a library open (the welcome screen
    // gates every key it does not handle).
    let (_dir, engine) = seed_library("viewer_zoom_keys", 1);
    let mut cx = crate::tests::TestApp::single();
    let (view, cx) = app_with_library(&mut cx, engine);
    let sample = get_sample_items();
    let photo = sample[0].clone();

    view.update(cx, |app, cx| {
        app.select_item_at(photo.clone(), 0, cx);
        app.open_viewer(photo.clone(), cx);
        assert_eq!(app.viewer_state.zoom_label(), "Fit");

        // Preset actions drive both scale and HUD text.
        app.apply_viewer_zoom_preset(ViewerZoomPreset::TwoX, cx);
        assert_eq!(app.viewer_state.zoom_scale, 2.0);
        assert_eq!(app.viewer_state.zoom_label(), "200%");
        app.apply_viewer_zoom_preset(ViewerZoomPreset::FourX, cx);
        assert_eq!(app.viewer_state.zoom_scale, 4.0);
        assert_eq!(app.viewer_state.zoom_label(), "400%");
        app.apply_viewer_zoom_preset(ViewerZoomPreset::Fit, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(app.viewer_state.zoom_label(), "Fit");

        // Reset restores fit and clears pan without leaving drag state behind.
        app.set_viewer_zoom(2.0, cx);
        app.start_viewer_drag(10.0, 10.0, cx);
        app.drag_viewer_to(40.0, 30.0, cx);
        assert_eq!(app.viewer_state.pan_offset, (30.0, 20.0));
        app.reset_viewer_zoom(cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MIN_SCALE);
        assert_eq!(app.viewer_state.pan_offset, (0.0, 0.0));
        assert!(!app.viewer_state.is_dragging);

        // Existing stepping behavior is unchanged.
        app.zoom_in_viewer(cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.25);
        app.zoom_out_viewer(cx);
        assert_eq!(app.viewer_state.zoom_scale, 1.0);
        app.set_viewer_zoom(99.0, cx);
        assert_eq!(app.viewer_state.zoom_scale, ZOOM_MAX_SCALE);
    });
    cx.run_until_parked();

    // Keyboard: ⌘1/⌘2/⌘4 presets plus preserved +, -, ⌘0 and Z paths.
    cx.update(|window, app_cx| {
        view.update(app_cx, |app, cx| {
            app.select_item_at(photo.clone(), 0, cx);
            app.open_viewer(photo.clone(), cx);

            // 3000px long edge on a 2x screen over a 900pt fit: 100% = 1.667x.
            app.viewer_state.canvas = crate::views::viewer_state::ViewerCanvas::new(
                (1200.0, 600.0),
                (600.0, 300.0),
                1.5,
                3000.0,
                2.0,
            );
            let actual = 1500.0 / 900.0;
            press_viewer_key(app, "cmd-1", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - actual).abs() < 1e-4,
                "⌘1 must select actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "100%");
            press_viewer_key(app, "cmd-2", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - 2.0 * actual).abs() < 1e-4,
                "⌘2 must select 200% of actual pixels"
            );
            assert_eq!(app.viewer_state.zoom_label(), "200%");
            press_viewer_key(app, "cmd-4", window, cx);
            assert_eq!(app.viewer_state.zoom_label(), "400%", "⌘4 must select 400%");
            press_viewer_key(app, "cmd-0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "⌘0 must select Fit");

            press_viewer_key(app, "=", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.25, "+ must still step in");
            press_viewer_key(app, "cmd-0", window, cx);
            assert_eq!(app.viewer_state.zoom_scale, 1.0, "⌘0 must still reset");
            press_viewer_key(app, "z", window, cx);
            assert!(
                (app.viewer_state.zoom_scale - actual).abs() < 1e-4,
                "Z must toggle Fit to actual pixels"
            );
            press_viewer_key(app, "z", window, cx);
            assert_eq!(
                app.viewer_state.zoom_scale, 1.0,
                "Z must still toggle to Fit"
            );
            press_viewer_key(app, "-", window, cx);
            assert_eq!(
                app.viewer_state.zoom_scale, 1.0,
                "- must still step down and clamp at Fit"
            );
        });
    });
    cx.run_until_parked();
}

#[core::prelude::v1::test]
fn test_viewer_fit_button_floats_below_the_top_bar() {
    let (_temp_dir, engine) = seed_library("viewer_zoom_hud", 2);
    assert!(engine.total_count() >= 2);

    let mut cx = crate::tests::TestApp::single();
    let (view, vcx) = cx.add_window_view(|_window, cx| LuminaApp::new(cx));
    let photo = get_sample_items()[0].clone();

    view.update(vcx, |app, cx| {
        app.grid_state = Some(crate::views::grid::GridState::new(engine.clone()));
        app.select_item_at(photo.clone(), 0, cx);
        app.open_viewer(photo.clone(), cx);
        app.window_bounds = Some((0.0, 0.0, 1024.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1024.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    // The bounds observer fires on the resize frame; draw again so the top bar
    // renders with the observed width.
    render_viewer_ui(vcx);

    for selector in ["viewer-top-bar", "viewer-position", "viewer-back"] {
        assert!(
            vcx.debug_bounds(selector).is_some(),
            "{selector} must be painted in the viewer at 1024x768"
        );
    }
    assert!(
        vcx.debug_bounds("viewer-zoom-fit").is_none(),
        "at Fit there is no zoom chrome"
    );
    view.update(vcx, |app, cx| {
        app.apply_viewer_zoom_preset(crate::views::viewer_state::ViewerZoomPreset::TwoX, cx)
    });
    render_viewer_ui(vcx);

    let position = vcx.debug_bounds("viewer-position").unwrap();
    let hud = vcx
        .debug_bounds("viewer-zoom-fit")
        .expect("zoomed, the Fit to window button must show");
    let top_bar = vcx.debug_bounds("viewer-top-bar").unwrap();
    // The centre shows only "N / M" (the date moved to the
    // panel's When line); it must stay readable at 1024x768.
    assert!(
        position.size.width > gpui::px(20.0),
        "the position must retain readable width at 1024x768, got {:?}",
        position.size.width
    );
    assert!(
        hud.top() >= top_bar.bottom(),
        "the Fit button must float below the top bar: hud={hud:?} top_bar={top_bar:?}"
    );
    assert!(
        hud.left() >= top_bar.left() && hud.right() <= top_bar.right(),
        "the Fit button must stay inside the window width at 1024x768"
    );

    // At 1400px the top bar and title continue to render cleanly.
    view.update(vcx, |app, cx| {
        app.window_bounds = Some((0.0, 0.0, 1400.0, 768.0));
        cx.notify();
    });
    vcx.simulate_resize(gpui::size(gpui::px(1400.0), gpui::px(768.0)));
    render_viewer_ui(vcx);
    render_viewer_ui(vcx);
    let position_wide = vcx.debug_bounds("viewer-position").unwrap();
    assert_eq!(
        position_wide.size.width, position.size.width,
        "the position keeps its width at 1400px"
    );

    click(vcx, "viewer-zoom-fit");
    view.update(vcx, |app, _| {
        assert_eq!(
            app.viewer_state.zoom_scale, 1.0,
            "Fit to window must return to Fit"
        );
    });
}
