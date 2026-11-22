//! Rating, colour label and pick/reject controls for the context menu, and
//! the grid's flag badge. Each control acts on the selection and never
//! moves it (only the viewer's keys auto-advance, see `app/culling.rs`).

use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use gpui::*;
use lumina_core::{Flag, MediaItem};

/// Rejected cards are drawn at this opacity so a cull reads at a glance.
/// It replaces the Recently Deleted dimming (0.6) rather than multiplying
/// with it: a rejected item in the trash is drawn at this value.
pub const REJECTED_OPACITY: f32 = 0.4;

/// A context-menu row: a small caption on the left, controls on the right.
fn mark_row(caption: impl Into<SharedString>, controls: impl IntoElement) -> Div {
    div()
        .px_2p5()
        .py_1()
        .rounded(theme::RADIUS_SM)
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_size(theme::TEXT_SMALL)
                .text_color(theme::t1())
                .child(caption.into()),
        )
        .child(controls)
}

/// Five stars; a click rates the selection (the current rating clears).
pub fn rating_row(app: &LuminaApp, item: &MediaItem, cx: &mut Context<LuminaApp>) -> Div {
    let current_rating = app.get_item_rating(item);
    let star_pop = app.mark_pop.star(Some(item.id));
    mark_row(
        "Rating",
        div()
            .flex()
            .flex_row()
            .items_center()
            .children((1..=5).map(|star_val| {
                let is_filled = star_val <= current_rating;
                div()
                    .id(ElementId::NamedInteger(
                        "context-rating-star".into(),
                        star_val as u64,
                    ))
                    .debug_selector(move || format!("context-rating-star-{star_val}"))
                    .size(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .child(crate::motion::pop(
                        render_icon(
                            if is_filled {
                                Icon::StarFilled
                            } else {
                                Icon::Star
                            },
                            px(12.0),
                            if is_filled {
                                theme::rating()
                            } else {
                                theme::rating_unset()
                            },
                        ),
                        star_pop
                            .clone()
                            .filter(|(s, _)| *s == star_val)
                            .map(|(_, id)| id),
                    ))
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.set_rating_selected(star_val, cx);
                    }))
            })),
    )
}

/// The colour label swatches; a click labels the selection.
pub fn label_row(app: &LuminaApp, item: &MediaItem, cx: &mut Context<LuminaApp>) -> Div {
    let current_label = app.item_color_labels.get(&item.id).copied().unwrap_or(0);
    mark_row(
        "Label",
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(2.0))
            .children(theme::label_options().into_iter().map(|label| {
                let label_idx = label.0;
                theme::label_swatch("context-label", label, current_label == label_idx).on_click(
                    cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.set_color_label_selected(label_idx, cx);
                    }),
                )
            })),
    )
}

/// Pick and Reject buttons; the one the item has is drawn on, and a click
/// on it clears the flag.
pub fn flag_row(app: &LuminaApp, item: &MediaItem, cx: &mut Context<LuminaApp>) -> Div {
    let current = app.item_flag(item.id);
    let button = |flag: Flag, selector: &'static str, key: &str, shortcut: &str| {
        let label = t(key);
        let on = current == flag;
        if on {
            theme::control_on(selector)
        } else {
            theme::control(selector)
        }
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .h(px(22.0))
        .tooltip(theme::tooltip_label(format!("{label} ({shortcut})")))
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_flag_selected(flag, cx);
        }))
    };
    mark_row(
        t("context.flag"),
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(button(
                Flag::Pick,
                "context-flag-pick",
                "command.flag_pick",
                "P",
            ))
            .child(button(
                Flag::Reject,
                "context-flag-reject",
                "command.flag_reject",
                "X",
            )),
    )
}

/// Badge for a picked or rejected item; callers position it (the grid puts
/// it in the tile's top-right row, left of the heart and missing badge).
pub fn flag_badge(item_id_num: usize, flag: Flag) -> AnyElement {
    let (icon, color, label) = match flag {
        Flag::Pick => (Icon::Check, theme::accent(), "Picked"),
        Flag::Reject => (Icon::X, theme::danger(), "Rejected"),
        Flag::Unflagged => return Empty.into_any_element(),
    };
    div()
        .id(("grid-flag-badge", item_id_num as u64))
        .debug_selector(move || format!("grid-flag-badge-{item_id_num}"))
        .role(Role::Image)
        .aria_label(label)
        .size(px(20.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::media_scrim())
        .flex()
        .items_center()
        .justify_center()
        .tooltip(theme::tooltip_label(label))
        .child(render_icon(icon, px(12.0), color))
        .into_any_element()
}
