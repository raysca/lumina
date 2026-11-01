use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaItem;

use crate::app::types::{format_bytes, month_name};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

pub struct ViewerTopBar;

/// Transparent 28px icon button with a tooltip.
fn bar_icon_button(
    id: &'static str,
    icon: Icon,
    tooltip: &'static str,
    color: Rgba,
    on: bool,
) -> Stateful<Div> {
    bar_button(id, tooltip, on).child(render_icon(icon, px(16.0), color))
}

/// `bar_icon_button` without its icon, for an icon that animates.
fn bar_button(id: &'static str, tooltip: &'static str, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(tooltip))
        .size(px(28.0))
        .rounded(theme::RADIUS_MD)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |b| b.bg(theme::selection_bg()))
        .hover(|s| s.bg(theme::bg_control_hover()))
}

/// Window width below which the top bar drops the breadcrumb date and specs
/// line so the filename stays readable.
const COMPACT_WINDOW_WIDTH: f32 = 1400.0;
/// Width assumed before window bounds have been observed.
const DEFAULT_WINDOW_WIDTH: f32 = 1440.0;

impl ViewerTopBar {
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

        let is_inspector_open = this.inspector_open;

        let month_str = month_name(item.month);
        let date_prefix = if item.year > 0 {
            format!("{} {}", month_str, item.year)
        } else {
            "Undated".to_string()
        };
        let is_fav = item.is_favorite;

        // At narrow widths the breadcrumb date and specs line yield space so the
        // filename stays readable at 1024px.
        let window_width = this
            .window_bounds
            .map(|(_, _, width, _)| width)
            .unwrap_or(DEFAULT_WINDOW_WIDTH);
        let compact = window_width < COMPACT_WINDOW_WIDTH;

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
                    .child(specs_line(item)),
            );
        }

        // "3 of 5" in the current view; hidden when the viewer has no grid index.
        if let Some(index) = this.selected_index {
            center_section = center_section.child(
                div()
                    .id("viewer-position")
                    .debug_selector(|| "viewer-position".to_string())
                    .ml_2()
                    .flex_shrink_0()
                    .text_color(theme::text_subtle())
                    .child(format!("{} of {}", index + 1, this.displayed_count())),
            );
        }

        div()
            .id("viewer-top-bar")
            .debug_selector(|| "viewer-top-bar".to_string())
            .on_hover(cx.listener(|this, on: &bool, _w, _cx| this.set_pointer_on_hud(*on)))
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
                            .rounded(theme::RADIUS_MD)
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
            // Right Section: Favorite and Info only (v3). Open, Reveal, Edit
            // and Rotate live in the viewer's right-click menu.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .flex_shrink_0()
                    .child(
                        bar_button(
                            "viewer-fav-btn",
                            if is_fav {
                                "Unfavorite (F)"
                            } else {
                                "Favorite (F)"
                            },
                            false,
                        )
                        .child(crate::motion::pop(
                            render_icon(
                                if is_fav {
                                    Icon::HeartFilled
                                } else {
                                    Icon::Heart
                                },
                                px(16.0),
                                if is_fav {
                                    theme::rating()
                                } else {
                                    theme::text_muted()
                                },
                            ),
                            this.mark_pop.heart(Some(item.id)),
                        ))
                        .on_click(cx.listener(
                            |this, _e: &ClickEvent, _w, cx| {
                                this.toggle_favorite_selected(cx);
                            },
                        )),
                    )
                    .child(
                        bar_icon_button(
                            "viewer-info-btn",
                            Icon::Info,
                            "Info (I)",
                            if is_inspector_open {
                                theme::selection_fg()
                            } else {
                                theme::text_muted()
                            },
                            is_inspector_open,
                        )
                        .on_click(cx.listener(
                            |this, _e: &ClickEvent, _w, cx| {
                                this.toggle_inspector(cx);
                            },
                        )),
                    ),
            )
    }
}

/// "4032 × 3024 · HEIC · 3.1 MB" from what the catalog actually knows: no
/// guessed codec, frame rate or dimensions.
pub(crate) fn specs_line(item: &lumina_core::MediaItem) -> String {
    let mut parts = Vec::new();
    if item.width > 0 && item.height > 0 {
        parts.push(format!("{} × {}", item.width, item.height));
    }
    if let Some(ext) = item.path.extension().and_then(|e| e.to_str()) {
        parts.push(ext.to_uppercase());
    }
    parts.push(format_bytes(item.file_size));
    parts.join(" · ")
}
