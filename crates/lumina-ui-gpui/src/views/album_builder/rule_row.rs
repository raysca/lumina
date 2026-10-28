use gpui::*;
use lumina_core::{RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use super::dropdowns;
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_rule_row(
    app: &LuminaApp,
    idx: usize,
    rule: &SmartAlbumRule,
    _total_rules: usize,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let is_photo_field = rule.field == RuleField::IsFavorite || rule.field == RuleField::MediaType;

    let field_label = if is_photo_field {
        "Photo"
    } else {
        match rule.field {
            RuleField::Tag => "Tag",
            RuleField::CameraMake => "Camera Make",
            RuleField::CameraModel => "Camera Model",
            RuleField::Lens => "Lens",
            RuleField::City => "City",
            RuleField::Year => "Year",
            RuleField::Iso => "ISO Speed",
            RuleField::FNumber => "Aperture (f/)",
            RuleField::Rating => "Rating (Stars)",
            RuleField::ColorLabel => "Color Label",
            _ => "Attribute",
        }
    };

    let op_label = match rule.operator {
        RuleOperator::Equals => "is",
        RuleOperator::NotEquals => "is not",
        RuleOperator::Contains => "contains",
        RuleOperator::NotContains => "does not contain",
        RuleOperator::GreaterThan => "greater than",
        RuleOperator::LessThan => "less than",
        RuleOperator::IsEmpty => "is empty",
        RuleOperator::IsNotEmpty => "is not empty",
        _ => "matches",
    };

    let photo_choice_label = if is_photo_field {
        if rule.field == RuleField::IsFavorite {
            if rule.value == RuleValue::Bool(true) {
                "favourite"
            } else {
                "not favourite"
            }
        } else if let RuleValue::Text(ref s) = rule.value {
            match s.as_str() {
                "video" => "video",
                "raw" => "raw",
                "image" => "still photo",
                _ => "media",
            }
        } else {
            "favourite"
        }
    } else {
        ""
    };

    let is_field_open = app.album_draft.field_dropdown == Some(idx);
    let is_op_open = app.album_draft.operator_dropdown == Some(idx);
    let is_val_open = app.album_draft.value_dropdown == Some(idx);
    let is_val_editing = app.album_draft.rule_value_editing == Some(idx);

    let is_empty_or_not_empty =
        rule.operator == RuleOperator::IsEmpty || rule.operator == RuleOperator::IsNotEmpty;

    div()
        .id(ElementId::NamedInteger("rule-row".into(), idx as u64))
        .w_full()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                // 1. Field Selector [ City ∨ ]
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "field-dropdown-btn".into(),
                            idx as u64,
                        ))
                        .w(px(160.0))
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()))
                        .border_1()
                        .border_color(if is_field_open {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(field_label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(if is_field_open { "▴" } else { "∨" }),
                        )
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.toggle_draft_field_dropdown(idx, cx);
                        })),
                )
                // 2. Operator Selector [ is ∨ ]
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "op-dropdown-btn".into(),
                            idx as u64,
                        ))
                        .w(px(140.0))
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()))
                        .border_1()
                        .border_color(if is_op_open {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::NORMAL)
                                .text_color(theme::text())
                                .child(op_label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(if is_op_open { "▴" } else { "∨" }),
                        )
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.toggle_draft_operator_dropdown(idx, cx);
                        })),
                )
                // 3. Value Selector / Input
                .child(if is_photo_field {
                    div()
                        .id(ElementId::NamedInteger("photo-val-btn".into(), idx as u64))
                        .flex_1()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()))
                        .border_1()
                        .border_color(if is_val_open {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(photo_choice_label),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(if is_val_open { "▴" } else { "∨" }),
                        )
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.toggle_draft_value_dropdown(idx, cx);
                        }))
                        .into_any_element()
                } else if is_empty_or_not_empty {
                    div()
                        .flex_1()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_pane())
                        .border_1()
                        .border_color(theme::border())
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("—")
                        .into_any_element()
                } else if is_val_editing {
                    div()
                        .id(ElementId::NamedInteger(
                            "val-editing-input".into(),
                            idx as u64,
                        ))
                        .flex_1()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_viewer())
                        .border_1()
                        .border_color(theme::accent())
                        .rounded_lg()
                        .cursor_text()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(if app.album_draft.rule_value_buffer.is_empty() {
                                    "|".to_string()
                                } else {
                                    format!("{}|", app.album_draft.rule_value_buffer)
                                }),
                        )
                        .into_any_element()
                } else {
                    let val_str = match &rule.value {
                        RuleValue::Text(s) => {
                            if s.is_empty() {
                                "(click to edit)".to_string()
                            } else {
                                s.clone()
                            }
                        }
                        RuleValue::Number(n) => format!("{:.1}", n),
                        RuleValue::Integer(i) => format!("{}", i),
                        RuleValue::Bool(b) => format!("{}", b),
                        RuleValue::Range(min, max) => format!("{} - {}", min, max),
                    };

                    div()
                        .id(ElementId::NamedInteger(
                            "val-display-btn".into(),
                            idx as u64,
                        ))
                        .flex_1()
                        .h(px(34.0))
                        .px_3()
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()))
                        .border_1()
                        .border_color(theme::border())
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::text())
                                .child(val_str),
                        )
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.start_editing_rule_value(idx, cx);
                        }))
                        .into_any_element()
                })
                // 4. Action Buttons: [ − ] and [ + ]
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "delete-rule-btn".into(),
                            idx as u64,
                        ))
                        .w(px(30.0))
                        .h(px(30.0))
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()).text_color(theme::text()))
                        .border_1()
                        .border_color(theme::border())
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("−")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.remove_draft_rule(idx, cx);
                        })),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "insert-rule-btn".into(),
                            idx as u64,
                        ))
                        .w(px(30.0))
                        .h(px(30.0))
                        .bg(theme::bg_pane())
                        .hover(|s| s.bg(theme::bg_raised()).text_color(theme::text()))
                        .border_1()
                        .border_color(theme::border())
                        .rounded_lg()
                        .cursor_pointer()
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child("+")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.insert_draft_rule_after(idx, cx);
                        })),
                ),
        )
        // Inline Dropdown Tray when active
        .child(if is_field_open {
            dropdowns::render_field_dropdown_panel(idx, rule, is_photo_field, cx).into_any_element()
        } else if is_op_open {
            dropdowns::render_operator_dropdown_panel(idx, rule, is_photo_field, cx)
                .into_any_element()
        } else if is_val_open {
            dropdowns::render_value_dropdown_panel(idx, photo_choice_label, cx).into_any_element()
        } else {
            Empty.into_any_element()
        })
}
