//! Compare / survey view: the panes of `app/present.rs` in a 1×2, 2×2 or
//! 3×3 grid over the browser, sharing one zoom/pan.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{Flag, MediaItem};

use crate::app::present::{columns, CompareState};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// The compare overlay, or nothing when compare is closed. Painted below the
/// viewer, so ↵ shows the viewer on top and its Esc comes back here.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(state) = app.compare.as_ref() else {
        return Empty.into_any_element();
    };
    let cols = columns(state.items.len());
    let rows = state.items.chunks(cols).enumerate().map(|(row, chunk)| {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_row()
            .gap_2()
            .children(
                chunk
                    .iter()
                    .enumerate()
                    .map(|(col, item)| pane(app, state, row * cols + col, item, cx)),
            )
            // Keep a short last row's panes the size of the others.
            .children((chunk.len()..cols).map(|_| div().flex_1()))
    });
    let rows: Vec<Div> = rows.collect();
    div()
        .id("compare-view")
        .debug_selector(|| "compare-view".to_string())
        .role(Role::Group)
        .aria_label(t("compare.title"))
        .absolute()
        .inset_0()
        // The grid and toolbar beneath must not take clicks.
        .occlude()
        .bg(theme::bg_stage())
        .flex()
        .flex_col()
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                this.compare_view(cx, |view| view.end_drag());
            }),
        )
        // The banner and toast belong to compare while it covers the grid:
        // Esc dismisses a banner the user can see. Settings over compare
        // draws its own banner.
        .when(
            app.error_message.is_some() && !app.preferences_open,
            |view| {
                view.child(
                    div()
                        .debug_selector(|| "compare-banner".to_string())
                        .child(app.render_error_banner(cx)),
                )
            },
        )
        .child(header(state, cx))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_2()
                .flex()
                .flex_col()
                .children(rows),
        )
        // Settings over compare draws the one toast itself, above it.
        .when(app.toast.is_visible() && !app.preferences_open, |view| {
            view.child(
                div()
                    .debug_selector(|| "compare-toast".to_string())
                    .absolute()
                    .inset_0()
                    .child(app.render_toast_overlay(cx)),
            )
        })
        .into_any_element()
}

fn header(state: &CompareState, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .h(px(44.0))
        .flex_none()
        .px_3()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(theme::line())
        .child(
            div()
                .text_size(theme::TEXT_BODY)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(format!("{} ({})", t("compare.title"), state.items.len())),
        )
        .child(
            div()
                .flex_1()
                .text_size(theme::TEXT_SMALL)
                .text_color(theme::t2())
                .child(t("compare.hint")),
        )
        .child(
            theme::control("compare-close")
                .debug_selector(|| "compare-close".to_string())
                .role(Role::Button)
                .aria_label(t("compare.close"))
                .tooltip(theme::tooltip_label(format!(
                    "{} (Esc)",
                    t("compare.close")
                )))
                .child(render_icon(Icon::X, theme::ICON_XS, theme::t1()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.close_compare(cx);
                })),
        )
}

fn pane(
    app: &LuminaApp,
    state: &CompareState,
    index: usize,
    item: &MediaItem,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let id = item.id.0;
    let focused = index == state.focus;
    let scale = state.view.zoom_scale;
    let (pan_x, pan_y) = state.view.pan_offset;
    let flag = app.item_flag(item.id);
    let stars = app.item_ratings.get(&item.id).copied().unwrap_or(0);
    let marks = [
        match flag {
            Flag::Pick => t("command.flag_pick"),
            Flag::Reject => t("command.flag_reject"),
            Flag::Unflagged => String::new(),
        },
        "★".repeat(stars as usize),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    let label = if marks.is_empty() {
        item.file_name.clone()
    } else {
        format!("{}, {marks}", item.file_name)
    };
    let media = match state.textures.get(&item.id) {
        Some(texture) => {
            let image = img(texture.clone())
                .id(("compare-media", id))
                .debug_selector(move || format!("compare-media-{id}"))
                .object_fit(ObjectFit::Contain);
            if scale > 1.0 {
                image
                    .flex_none()
                    .relative()
                    .left(px(pan_x))
                    .top(px(pan_y))
                    .w(relative(scale))
                    .h(relative(scale))
                    .into_any_element()
            } else {
                image.size_full().into_any_element()
            }
        }
        None if state.failed.contains(&item.id) => div()
            .text_size(theme::TEXT_BODY)
            .text_color(theme::t2())
            .child(t("viewer.preview_failed"))
            .into_any_element(),
        None => Empty.into_any_element(),
    };
    div()
        .id(("compare-pane", id))
        .debug_selector(move || format!("compare-pane-{id}"))
        .role(Role::Image)
        .aria_label(label)
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .rounded(theme::RADIUS_LG)
        .overflow_hidden()
        .bg(theme::bg_media())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, _w, cx| {
                this.focus_compare_pane(index, cx);
                let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                this.compare_view(cx, |view| view.start_drag(x, y));
            }),
        )
        .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _w, cx| {
            if this.compare.as_ref().is_some_and(|s| s.view.is_dragging) {
                let (x, y) = (f32::from(e.position.x), f32::from(e.position.y));
                this.compare_view(cx, |view| view.drag_to(x, y));
            }
        }))
        .on_scroll(cx.listener(|this, e: &ScrollEvent, w, cx| {
            let d = e.delta.pixel_delta(w.line_height());
            let (dx, dy) = (f32::from(d.x), f32::from(d.y));
            if dx.is_finite() && dy.is_finite() {
                this.compare_view(cx, |view| view.pan_by(dx, dy));
            }
        }))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center()
                .child(media),
        )
        .child(
            div()
                .flex_none()
                .px_2()
                .py_1()
                .flex()
                .flex_row()
                .gap_2()
                .text_size(theme::TEXT_SMALL)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_color(theme::t2())
                        .child(item.file_name.clone()),
                )
                .child(div().text_color(theme::accent()).child(marks)),
        )
        .when(focused, |pane| {
            pane.child(
                div()
                    .debug_selector(|| "compare-pane-focused".to_string())
                    .absolute()
                    .inset_0()
                    .rounded(theme::RADIUS_LG)
                    .border_2()
                    .border_color(theme::accent()),
            )
        })
        .into_any_element()
}
