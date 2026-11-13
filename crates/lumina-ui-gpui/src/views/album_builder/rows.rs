//! Condition rows and the shared picker / text field / popup controls.

use crate::app::text_input::FieldEditor;
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::{
    field_icon, field_label, operator_label, rating_value_label, DraftFocus, DraftRow, ValueKind,
};
use crate::app::text_input::TextTarget;
use crate::i18n::stars;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use lumina_core::RuleField;

/// Border for a control: the neutral focus ring when focused. Invalid values outline their
/// whole row instead (see `render_row`).
fn ring(focused: bool) -> Rgba {
    if focused {
        theme::focus_ring()
    } else {
        theme::line()
    }
}

/// Text that fills its control and is cut with "…" only when it overflows.
/// GPUI truncates by summing per-character widths, which can exceed the
/// shaped width a content-sized box gets; without filling the control the
/// last character (a label's last letter) was dropped.
fn fill_and_truncate(text: Div) -> Div {
    text.flex_1().min_w_0().truncate()
}

/// A popup button showing its current value, after an optional icon.
pub(super) fn picker(
    selector: String,
    icon: Option<Icon>,
    label: impl Into<SharedString>,
    focused: bool,
    target: DraftFocus,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    theme::control(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .justify_between()
        .gap_2()
        .border_color(ring(focused))
        .children(icon.map(|icon| render_icon(icon, px(14.0), theme::t2())))
        .child(fill_and_truncate(div()).child(label.into()))
        .child(render_icon(Icon::ChevronDown, px(13.0), theme::t2()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.toggle_draft_popup(target, cx);
        }))
}

/// A text field drawn from app state; while focused it draws `input`, the
/// field input that takes the typing (app/text_input.rs).
pub(super) fn text_field(
    selector: String,
    text: &str,
    placeholder: &str,
    focused: bool,
    input: Option<FieldEditor>,
    target: DraftFocus,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    let text_sel = format!("{selector}-text");
    let content = match (input, text.is_empty()) {
        (Some(input), _) => div().child(input),
        (None, true) => div().text_color(theme::t2()).child(placeholder.to_string()),
        (None, false) => div().child(text.to_string()),
    };
    theme::field(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .border_color(ring(focused))
        .child(fill_and_truncate(content).debug_selector(move || text_sel.clone()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_draft_focus(target, cx);
        }))
}

/// The open popup's options, in flow under the control that opened it.
pub(super) fn render_popup(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(popup) = app.album_draft.popup else {
        return Empty.into_any_element();
    };
    let options = app.draft_popup_options(popup.target);
    let empty = options.is_empty();
    theme::menu("album-builder-popup")
        .debug_selector(|| "album-builder-popup".to_string())
        .max_h(px(200.0))
        .overflow_y_scroll()
        .children(options.into_iter().enumerate().map(|(i, label)| {
            theme::menu_item(
                ElementId::NamedInteger("album-builder-option".into(), i as u64),
                i == popup.highlight,
            )
            .debug_selector(move || format!("album-builder-option-{i}"))
            .child(label)
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                this.pick_draft_option(i, cx);
            }))
        }))
        .when(empty, |menu| {
            menu.child(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(theme::t2())
                    .child("No tags in this library yet"),
            )
        })
        .into_any_element()
}

/// A condition row's control: 38 tall, bg-0, r9, Inter 13.
fn row_control(control: Stateful<Div>) -> Stateful<Div> {
    control
        .h(px(38.0))
        .px(px(12.0))
        .rounded(px(9.0))
        .bg(theme::bg_0())
        .text_size(px(13.0))
}

pub(super) fn square_button(
    selector: String,
    icon: Icon,
    tooltip: &'static str,
    focused: bool,
    on_click: impl Fn(&mut LuminaApp, &mut Context<LuminaApp>) + 'static,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    theme::control(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .flex_none()
        .size(px(34.0))
        .rounded(px(8.0))
        .bg(theme::bg_1())
        .justify_center()
        .border_color(ring(focused))
        .tooltip(theme::tooltip_label(tooltip))
        .child(render_icon(icon, px(14.0), theme::t2()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| on_click(this, cx)))
}

/// Five stars, filled up to the value, then "4 stars & up". A click sets the
/// rating; with the control focused ←/→ step it (app/keymap/modals.rs).
fn rating_stars(
    i: usize,
    row: &DraftRow,
    focused: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let value: i64 = row.text.parse().unwrap_or(0);
    let buttons = (1..=5i64).map(|n| {
        let (icon, color) = if n <= value {
            (Icon::StarFilled, theme::accent())
        } else {
            (Icon::Star, theme::t3())
        };
        div()
            .id(ElementId::Name(format!("album-rule-star-{i}-{n}").into()))
            .debug_selector(move || format!("album-rule-star-{i}-{n}"))
            .role(Role::Button)
            .aria_label(stars(n))
            .cursor_pointer()
            .child(render_icon(icon, px(15.0), color))
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                // The value control's own click would only refocus it.
                cx.stop_propagation();
                this.set_draft_rating(i, n, cx);
            }))
    });
    let sel = format!("album-rule-value-{i}");
    row_control(div().id(ElementId::Name(sel.clone().into())))
        .debug_selector(move || sel.clone())
        .flex_1()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .border_1()
        .border_color(ring(focused))
        .child(div().flex().flex_row().gap(px(4.0)).children(buttons))
        .child(
            div()
                .debug_selector(move || format!("album-rule-stars-label-{i}"))
                .text_size(px(12.0))
                .text_color(theme::t2())
                .child(rating_value_label(&row.operator, &row.text)),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_draft_focus(DraftFocus::Value(i), cx);
        }))
}

pub(super) fn render_row(
    app: &LuminaApp,
    i: usize,
    row: &DraftRow,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let focus = app.album_draft.focus;
    let invalid = row.to_rule().is_none();
    let kind = row.kind();
    let popup_here = app.album_draft.popup.is_some_and(|p| {
        matches!(
            p.target,
            DraftFocus::Field(r) | DraftFocus::Operator(r) | DraftFocus::Value(r) | DraftFocus::Unit(r)
                if r == i
        )
    });

    let value: AnyElement = match kind {
        ValueKind::None => div().flex_1().into_any_element(),
        ValueKind::Choice(_) if row.field == RuleField::Rating => {
            rating_stars(i, row, focus == DraftFocus::Value(i), cx).into_any_element()
        }
        ValueKind::Choice(_) | ValueKind::Tags => {
            let label = if row.text.is_empty() {
                kind.placeholder().to_string()
            } else {
                row.display_value()
            };
            // Kind shows what it picks: "video" shows Video, anything else (photo, raw) Image.
            let icon = (row.field == RuleField::MediaType).then(|| {
                if row.text == "video" {
                    Icon::Video
                } else {
                    Icon::Image
                }
            });
            row_control(picker(
                format!("album-rule-value-{i}"),
                icon,
                label,
                focus == DraftFocus::Value(i),
                DraftFocus::Value(i),
                cx,
            ))
            .flex_1()
            .into_any_element()
        }
        // A short centred number; the unit picker beside it fills the row.
        ValueKind::Period => row_control(text_field(
            format!("album-rule-value-{i}"),
            &row.text,
            kind.placeholder(),
            focus == DraftFocus::Value(i),
            app.field_input_for(TextTarget::AlbumDraft)
                .filter(|_| focus == DraftFocus::Value(i)),
            DraftFocus::Value(i),
            cx,
        ))
        .flex_none()
        .w(px(66.0))
        .justify_center()
        .text_center()
        .font_family(theme::FONT_MONO)
        .into_any_element(),
        _ => row_control(text_field(
            format!("album-rule-value-{i}"),
            &row.text,
            kind.placeholder(),
            focus == DraftFocus::Value(i),
            app.field_input_for(TextTarget::AlbumDraft)
                .filter(|_| focus == DraftFocus::Value(i)),
            DraftFocus::Value(i),
            cx,
        ))
        .flex_1()
        .into_any_element(),
    };

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .debug_selector(move || format!("album-rule-row-{i}"))
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                // The whole row is outlined while its value is invalid; the
                // transparent border keeps valid rows the same size, and the
                // negative margin keeps the outline out of the layout.
                .m(px(-4.0))
                .p(px(3.0))
                .rounded(theme::RADIUS_LG)
                .border_1()
                .border_color(if invalid {
                    theme::danger()
                } else {
                    rgba(0x00000000)
                })
                .child(
                    row_control(picker(
                        format!("album-rule-field-{i}"),
                        Some(field_icon(&row.field)),
                        field_label(&row.field),
                        focus == DraftFocus::Field(i),
                        DraftFocus::Field(i),
                        cx,
                    ))
                    .flex_none()
                    .w(px(210.0))
                    .font_weight(FontWeight::MEDIUM),
                )
                .child(
                    row_control(picker(
                        format!("album-rule-op-{i}"),
                        None,
                        operator_label(&row.field, &row.operator),
                        focus == DraftFocus::Operator(i),
                        DraftFocus::Operator(i),
                        cx,
                    ))
                    .flex_none()
                    .w(px(140.0))
                    .text_color(theme::t2()),
                )
                .child(value)
                .when(kind == ValueKind::Period, |el| {
                    el.child(
                        row_control(picker(
                            format!("album-rule-unit-{i}"),
                            None,
                            row.unit.label(),
                            focus == DraftFocus::Unit(i),
                            DraftFocus::Unit(i),
                            cx,
                        ))
                        .flex_1(),
                    )
                })
                .child(square_button(
                    format!("album-rule-remove-{i}"),
                    Icon::Minus,
                    "Remove condition",
                    focus == DraftFocus::Remove(i),
                    move |this, cx| this.remove_draft_row(i, cx),
                    cx,
                ))
                .child(square_button(
                    format!("album-rule-add-{i}"),
                    Icon::Plus,
                    "Add condition",
                    focus == DraftFocus::Add(i),
                    move |this, cx| this.add_draft_row_after(i, cx),
                    cx,
                )),
        )
        .when(popup_here, |el| el.child(render_popup(app, cx)))
}
