//! Condition rows and the shared picker / text field / popup controls.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::{field_label, operator_label, DraftFocus, DraftRow, ValueKind};
use crate::theme;
use crate::LuminaApp;

/// Border for a control: accent when focused. Invalid values outline their
/// whole row instead (see `render_row`).
fn ring(focused: bool) -> Rgba {
    if focused {
        theme::accent()
    } else {
        theme::border()
    }
}

/// A popup button showing its current value.
pub(super) fn picker(
    selector: String,
    label: impl Into<SharedString>,
    focused: bool,
    target: DraftFocus,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    theme::control(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .justify_between()
        .border_color(ring(focused))
        .child(div().truncate().child(label.into()))
        .child(div().text_color(theme::text_subtle()).child("⌄"))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.toggle_draft_popup(target, cx);
        }))
}

/// A text field drawn from app state; typing goes through the key handler.
pub(super) fn text_field(
    selector: String,
    text: &str,
    placeholder: &str,
    focused: bool,
    target: DraftFocus,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    let content = match (text.is_empty(), focused) {
        (true, false) => div()
            .text_color(theme::text_subtle())
            .child(placeholder.to_string()),
        (true, true) => div().child("|"),
        (false, true) => div().child(format!("{text}|")),
        (false, false) => div().child(text.to_string()),
    };
    theme::field(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .border_color(ring(focused))
        .child(content.truncate())
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
                    .text_color(theme::text_subtle())
                    .child("No tags in this library yet"),
            )
        })
        .into_any_element()
}

fn square_button(
    selector: String,
    glyph: &'static str,
    tooltip: &'static str,
    focused: bool,
    on_click: impl Fn(&mut LuminaApp, &mut Context<LuminaApp>) + 'static,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let sel = selector.clone();
    theme::control(ElementId::Name(selector.into()))
        .debug_selector(move || sel.clone())
        .w(px(28.0))
        .justify_center()
        .border_color(ring(focused))
        .tooltip(theme::tooltip_label(tooltip))
        .child(glyph)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| on_click(this, cx)))
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
    let can_remove = app.album_draft.rows.len() > 1;
    let popup_here = app.album_draft.popup.is_some_and(|p| {
        matches!(
            p.target,
            DraftFocus::Field(r) | DraftFocus::Operator(r) | DraftFocus::Value(r) | DraftFocus::Unit(r)
                if r == i
        )
    });

    let value: AnyElement = match kind {
        ValueKind::None => div().flex_1().into_any_element(),
        ValueKind::Choice(_) | ValueKind::Tags => {
            let label = if row.text.is_empty() {
                kind.placeholder().to_string()
            } else {
                row.display_value()
            };
            picker(
                format!("album-rule-value-{i}"),
                label,
                focus == DraftFocus::Value(i),
                DraftFocus::Value(i),
                cx,
            )
            .flex_1()
            .into_any_element()
        }
        _ => text_field(
            format!("album-rule-value-{i}"),
            &row.text,
            kind.placeholder(),
            focus == DraftFocus::Value(i),
            DraftFocus::Value(i),
            cx,
        )
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
                .gap_2()
                // The whole row is outlined while its value is invalid; the
                // transparent border keeps valid rows the same size.
                .p(px(3.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if invalid {
                    theme::danger_border()
                } else {
                    rgba(0x00000000)
                })
                .child(
                    picker(
                        format!("album-rule-field-{i}"),
                        field_label(&row.field),
                        focus == DraftFocus::Field(i),
                        DraftFocus::Field(i),
                        cx,
                    )
                    .w(px(130.0)),
                )
                .child(
                    picker(
                        format!("album-rule-op-{i}"),
                        operator_label(&row.field, &row.operator),
                        focus == DraftFocus::Operator(i),
                        DraftFocus::Operator(i),
                        cx,
                    )
                    .w(px(140.0)),
                )
                .child(value)
                .when(kind == ValueKind::Period, |el| {
                    el.child(
                        picker(
                            format!("album-rule-unit-{i}"),
                            row.unit.label(),
                            focus == DraftFocus::Unit(i),
                            DraftFocus::Unit(i),
                            cx,
                        )
                        .w(px(90.0)),
                    )
                })
                .when(can_remove, |el| {
                    el.child(square_button(
                        format!("album-rule-remove-{i}"),
                        "−",
                        "Remove condition",
                        focus == DraftFocus::Remove(i),
                        move |this, cx| this.remove_draft_row(i, cx),
                        cx,
                    ))
                })
                .child(square_button(
                    format!("album-rule-add-{i}"),
                    "+",
                    "Add condition",
                    focus == DraftFocus::Add(i),
                    move |this, cx| this.add_draft_row_after(i, cx),
                    cx,
                )),
        )
        .when(popup_here, |el| el.child(render_popup(app, cx)))
}
