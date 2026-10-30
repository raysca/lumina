use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::types::{format_bytes, month_name};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::{ViewerRotation, ViewerZoomPreset};

pub struct ViewerTopBar;

/// Window width below which the top bar drops the breadcrumb date and specs
/// line and switches open/reveal to icon actions so the zoom HUD cannot crowd
/// the filename.
const COMPACT_WINDOW_WIDTH: f32 = 1400.0;
/// Width assumed before window bounds have been observed.
const DEFAULT_WINDOW_WIDTH: f32 = 1440.0;

impl ViewerTopBar {
    fn render_zoom_button(
        preset: ViewerZoomPreset,
        active: bool,
        cx: &mut Context<LuminaApp>,
    ) -> Stateful<Div> {
        let id = match preset {
            ViewerZoomPreset::Fit => "viewer-zoom-fit",
            ViewerZoomPreset::Actual => "viewer-zoom-actual",
            ViewerZoomPreset::TwoX => "viewer-zoom-2x",
            ViewerZoomPreset::FourX => "viewer-zoom-4x",
        };
        let button = div()
            .id(id)
            .debug_selector(move || id.to_string())
            .tooltip(theme::tooltip_label(preset.tooltip()))
            .h(px(18.0))
            .px(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .text_size(px(10.0))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.apply_viewer_zoom_preset(preset, cx);
            }))
            .child(preset.label());

        if active {
            button
                .bg(theme::selection_bg())
                .text_color(theme::selection_fg())
        } else {
            button.text_color(theme::text_muted()).hover(|style| {
                style
                    .bg(theme::bg_control_hover())
                    .text_color(theme::text())
            })
        }
    }

    fn render_zoom_reset(cx: &mut Context<LuminaApp>) -> Stateful<Div> {
        div()
            .id("viewer-zoom-reset")
            .debug_selector(|| "viewer-zoom-reset".to_string())
            .tooltip(theme::tooltip_label("Reset zoom and pan (0)"))
            .h(px(18.0))
            .px(px(3.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(theme::text_muted())
            .hover(|style| style.bg(theme::bg_control_hover()))
            .child(render_icon(Icon::RotateCcw, px(11.0), theme::text_muted()))
            .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                this.reset_viewer_zoom(cx);
            }))
    }

    fn render_zoom_hud(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
        let state = this.viewer_state;
        let mut hud = div()
            .id("viewer-zoom-hud")
            .debug_selector(|| "viewer-zoom-hud".to_string())
            .h(px(24.0))
            .px(px(1.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(1.0))
            .rounded(px(6.0))
            .bg(theme::bg_control())
            .border_1()
            .border_color(theme::border())
            .child(
                div()
                    .id("viewer-zoom-label")
                    .debug_selector(|| "viewer-zoom-label".to_string())
                    .tooltip(theme::tooltip_label(
                        "Zoom as a percentage of actual pixels",
                    ))
                    .min_w(px(30.0))
                    .text_center()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(11.0))
                    .text_color(theme::text())
                    .child(state.zoom_label()),
            );

        for preset in ViewerZoomPreset::ALL {
            hud = hud.child(Self::render_zoom_button(
                preset,
                state.matches_preset(preset),
                cx,
            ));
        }

        hud.child(Self::render_zoom_reset(cx))
    }

    /// Quick-edit rotate control. It stays visible for video so the no-op is
    /// discoverable, but the tooltip states that video rotation is unsupported.
    fn render_rotate_button(
        direction: ViewerRotation,
        is_video: bool,
        cx: &mut Context<LuminaApp>,
    ) -> Stateful<Div> {
        let id = direction.debug_selector();
        let tooltip = if is_video {
            format!("{} (not supported for video)", direction.label())
        } else {
            direction.tooltip().to_string()
        };
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .tooltip(theme::tooltip_label(tooltip))
            .h(px(22.0))
            .px(px(2.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .bg(theme::bg_control())
            .border_1()
            .border_color(theme::border())
            .hover(|style| style.bg(theme::bg_control_hover()))
            .cursor_pointer()
            .text_color(theme::text_muted())
            .child(render_icon(
                match direction {
                    ViewerRotation::Left => Icon::RotateCcw,
                    ViewerRotation::Right => Icon::RotateCw,
                },
                px(13.0),
                theme::text_muted(),
            ))
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                this.rotate_viewer_selected(direction, cx);
            }))
    }

    pub fn render(
        this: &LuminaApp,
        item: &MediaItem,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let path = item.path.clone();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&item.file_name)
            .to_string();

        let is_video = item.media_type == MediaType::Video;
        let is_inspector_open = this.inspector_open;

        let month_str = month_name(item.month);
        let year_num = if item.year > 0 {
            item.year
        } else {
            this.active_year.unwrap_or(2022)
        };
        let date_prefix = format!("{} {}", month_str, year_num);

        let codec_label = if is_video {
            "H.264"
        } else {
            path.extension().and_then(|e| e.to_str()).unwrap_or("HEIC")
        };

        let fps_label = if is_video { " · 30 fps" } else { "" };
        let width = if item.width > 0 {
            item.width
        } else if is_video {
            1920
        } else {
            4032
        };
        let height = if item.height > 0 {
            item.height
        } else if is_video {
            1080
        } else {
            3024
        };
        let size_str = format_bytes(item.file_size);
        let is_fav = item.is_favorite;

        // The HUD adds a compact control group to the right cluster. At narrow
        // widths the breadcrumb date, specs line, and long action labels yield
        // space so the filename stays readable at 1024px.
        let window_width = this
            .window_bounds
            .map(|(_, _, width, _)| width)
            .unwrap_or(DEFAULT_WINDOW_WIDTH);
        let compact = window_width < COMPACT_WINDOW_WIDTH;
        let open_label = if is_video {
            if compact {
                "Open"
            } else {
                "Open in QuickTime"
            }
        } else if compact {
            "Open"
        } else {
            "Open in Preview"
        };
        let reveal_label = if compact {
            "Reveal"
        } else {
            "Reveal in Finder"
        };

        let mut center_section = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1p5()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden();

        // At compact widths the filename gets the whole center section. The
        // breadcrumb date is secondary information that remains in the inspector.
        if !compact {
            center_section = center_section
                .child(div().text_color(theme::text_muted()).child(date_prefix))
                .child(div().text_color(theme::text_disabled()).child(render_icon(
                    Icon::ChevronRight,
                    px(10.0),
                    theme::text_disabled(),
                )));
        }

        center_section = center_section.child(
            div()
                .id("viewer-filename")
                .debug_selector(|| "viewer-filename".to_string())
                .min_w(px(70.0))
                .truncate()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text())
                .child(file_name),
        );

        if !compact {
            center_section = center_section.child(
                div()
                    .id("viewer-specs")
                    .debug_selector(|| "viewer-specs".to_string())
                    .ml_2()
                    .min_w(px(0.0))
                    .truncate()
                    .text_color(theme::text_subtle())
                    .child(format!(
                        "{} × {} · {}{} · {}",
                        width, height, codec_label, fps_label, size_str
                    )),
            );
        }

        div()
            .id("viewer-top-bar")
            .debug_selector(|| "viewer-top-bar".to_string())
            .w_full()
            .h(px(52.0))
            .px_4()
            .bg(theme::bg_app())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .text_xs()
            .text_color(theme::text())
            // Left Section: Traffic Light Offset + the single Back control.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .flex_shrink_0()
                    // Traffic lights spacer when windowed
                    .child(div().w(px(64.0)))
                    // One visible way back to the grid; Escape stays equivalent.
                    .child(
                        div()
                            .id("viewer-back")
                            .debug_selector(|| "viewer-back".to_string())
                            .tooltip(theme::tooltip_label("Back to Library (Esc)"))
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .text_color(theme::text())
                            .child(render_icon(Icon::ChevronLeft, px(13.0), theme::text()))
                            .child(if compact {
                                Empty.into_any_element()
                            } else {
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Library")
                                    .into_any_element()
                            })
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.close_viewer(cx);
                            })),
                    ),
            )
            // Center Section: Breadcrumb, Filename, Specs Line
            .child(center_section)
            // Right Section: Zoom HUD, Quick Edit, Favorite, Info, Open, Reveal.
            // The gap tightens slightly so the added quick-edit controls keep the
            // HUD clear of the filename down to the 1024px minimum width.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .flex_shrink_0()
                    // Zoom HUD keeps the current zoom and presets discoverable.
                    .child(Self::render_zoom_hud(this, cx))
                    // Quick-edit rotation. Non-destructive: the catalog stores the
                    // quarter turn and the original file is never rewritten.
                    .child(
                        div()
                            .id("viewer-quick-edit")
                            .debug_selector(|| "viewer-quick-edit".to_string())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .children(ViewerRotation::ALL.into_iter().map(|direction| {
                                Self::render_rotate_button(direction, is_video, cx)
                            })),
                    )
                    // Favorite Heart Button
                    .child(
                        div()
                            .id("viewer-fav-btn")
                            .debug_selector(|| "viewer-fav-btn".to_string())
                            .tooltip(theme::tooltip_label("Toggle favorite"))
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .cursor_pointer()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_sm()
                            .text_color(if is_fav {
                                theme::rating()
                            } else {
                                theme::text_muted()
                            })
                            .child(render_icon(
                                if is_fav {
                                    Icon::HeartFilled
                                } else {
                                    Icon::Heart
                                },
                                px(14.0),
                                if is_fav {
                                    theme::rating()
                                } else {
                                    theme::text_muted()
                                },
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.toggle_favorite_selected(cx);
                            })),
                    )
                    // Info Button (with ⌘I label)
                    .child(
                        div()
                            .id("viewer-info-btn")
                            .debug_selector(|| "viewer-info-btn".to_string())
                            .px_2p5()
                            .py_1()
                            .bg(if is_inspector_open {
                                theme::selection_bg()
                            } else {
                                theme::bg_control()
                            })
                            .border_1()
                            .border_color(if is_inspector_open {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if is_inspector_open {
                                        theme::accent()
                                    } else {
                                        theme::text()
                                    })
                                    .child("Info"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::text_subtle())
                                    .child("⌘I"),
                            )
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.toggle_inspector(cx);
                            })),
                    )
                    // Open in QuickTime / Preview (with O label)
                    .child({
                        div()
                            .id("viewer-quicktime-btn")
                            .debug_selector(|| "viewer-quicktime-btn".to_string())
                            .tooltip(theme::tooltip_label(if is_video {
                                "Open in QuickTime (O)"
                            } else {
                                "Open in Preview (O)"
                            }))
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(if compact {
                                render_icon(Icon::ExternalLink, px(13.0), theme::text())
                                    .into_any_element()
                            } else {
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(open_label)
                                    .into_any_element()
                            })
                            .child(if compact {
                                Empty.into_any_element()
                            } else {
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::text_subtle())
                                    .child("O")
                                    .into_any_element()
                            })
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, _cx| {
                                this.open_selected_in_default_app();
                            }))
                    })
                    // Edit in External Editor (uses the Preferences app)
                    .child({
                        let p_clone = path.clone();
                        div()
                            .id("viewer-external-editor-btn")
                            .debug_selector(|| "viewer-external-editor-btn".to_string())
                            .tooltip(theme::tooltip_label("Edit in External Editor"))
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            // A short label keeps the right cluster narrow enough
                            // for the zoom HUD to clear the filename at 1400px;
                            // the tooltip and context menu carry the full name.
                            .child("Edit")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.open_item_in_external_editor(&p_clone, cx);
                            }))
                    })
                    // Reveal in Finder Button
                    .child({
                        let p_clone = path.clone();
                        div()
                            .id("viewer-finder-btn")
                            .debug_selector(|| "viewer-finder-btn".to_string())
                            .tooltip(theme::tooltip_label("Reveal in Finder"))
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            .child(if compact {
                                render_icon(Icon::Folder, px(13.0), theme::text())
                                    .into_any_element()
                            } else {
                                reveal_label.into_any_element()
                            })
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, _cx| {
                                this.reveal_item_in_finder(&p_clone);
                            }))
                    }),
            )
    }
}
