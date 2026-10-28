use gpui::*;
use lumina_core::{RuleField, RuleOperator, SmartAlbumRule};

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_field_dropdown_panel(
    idx: usize,
    rule: &SmartAlbumRule,
    is_photo_field: bool,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let available_fields = [
        (RuleField::City, "City"),
        (RuleField::FNumber, "Aperture (f/)"),
        (RuleField::Iso, "ISO Speed"),
        (RuleField::Tag, "Tag"),
        (RuleField::CameraMake, "Camera Make"),
        (RuleField::CameraModel, "Camera Model"),
        (RuleField::Lens, "Lens"),
        (RuleField::Year, "Year"),
        (RuleField::Rating, "Rating (Stars)"),
        (RuleField::ColorLabel, "Color Label"),
        (RuleField::IsFavorite, "Photo"),
    ];

    div()
        .id(ElementId::NamedInteger("field-tray".into(), idx as u64))
        .w_full()
        .p_3()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border())
        .rounded_lg()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SELECT ATTRIBUTE"),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger(
                            "close-field-tray".into(),
                            idx as u64,
                        ))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .hover(|s| s.text_color(theme::text()))
                        .child(render_icon(Icon::X, px(11.0), theme::text_subtle()))
                        .child("Close")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.album_draft.field_dropdown = None;
                            cx.notify();
                        })),
                ),
        )
        .child(div().flex().flex_row().flex_wrap().gap_1p5().children(
            available_fields.into_iter().map(|(f, label)| {
                let is_curr = if f == RuleField::IsFavorite {
                    is_photo_field
                } else {
                    rule.field == f
                };
                div()
                    .id(ElementId::Name(
                        format!("field-opt-{}-{}", idx, label).into(),
                    ))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(if is_curr {
                        theme::accent()
                    } else {
                        theme::bg_raised()
                    })
                    .hover(|s| {
                        s.bg(if is_curr {
                            theme::accent_hover()
                        } else {
                            theme::bg_control_hover()
                        })
                    })
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_curr {
                        theme::accent_fg()
                    } else {
                        theme::text()
                    })
                    .child(label)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.set_draft_rule_field(idx, f.clone(), cx);
                    }))
            }),
        ))
}

pub(super) fn render_operator_dropdown_panel(
    idx: usize,
    rule: &SmartAlbumRule,
    is_photo_field: bool,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let ops: Vec<(RuleOperator, &'static str)> = if is_photo_field {
        vec![
            (RuleOperator::Equals, "is"),
            (RuleOperator::NotEquals, "is not"),
        ]
    } else if rule.field == RuleField::Tag {
        vec![
            (RuleOperator::Equals, "is"),
            (RuleOperator::NotEquals, "is not"),
            (RuleOperator::Contains, "contains"),
            (RuleOperator::IsEmpty, "is empty"),
            (RuleOperator::IsNotEmpty, "is not empty"),
        ]
    } else if rule.field == RuleField::Iso
        || rule.field == RuleField::FNumber
        || rule.field == RuleField::Year
        || rule.field == RuleField::Rating
        || rule.field == RuleField::ColorLabel
    {
        vec![
            (RuleOperator::Equals, "is"),
            (RuleOperator::GreaterThan, "greater than"),
            (RuleOperator::LessThan, "less than"),
        ]
    } else {
        vec![
            (RuleOperator::Equals, "is"),
            (RuleOperator::NotEquals, "is not"),
            (RuleOperator::Contains, "contains"),
            (RuleOperator::NotContains, "does not contain"),
        ]
    };

    div()
        .id(ElementId::NamedInteger("op-tray".into(), idx as u64))
        .w_full()
        .p_3()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border())
        .rounded_lg()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SELECT OPERATOR"),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger("close-op-tray".into(), idx as u64))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .hover(|s| s.text_color(theme::text()))
                        .child(render_icon(Icon::X, px(11.0), theme::text_subtle()))
                        .child("Close")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.album_draft.operator_dropdown = None;
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(ops.into_iter().map(|(op, label)| {
                    let is_curr = rule.operator == op;
                    div()
                        .id(ElementId::Name(format!("op-opt-{}-{}", idx, label).into()))
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .bg(if is_curr {
                            theme::accent()
                        } else {
                            theme::bg_raised()
                        })
                        .hover(|s| {
                            s.bg(if is_curr {
                                theme::accent_hover()
                            } else {
                                theme::bg_control_hover()
                            })
                        })
                        .cursor_pointer()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if is_curr {
                            theme::accent_fg()
                        } else {
                            theme::text()
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.set_draft_rule_operator(idx, op.clone(), cx);
                        }))
                })),
        )
}

pub(super) fn render_value_dropdown_panel(
    idx: usize,
    photo_choice_label: &'static str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let photo_options = [
        ("favourite", "Favourite"),
        ("not favourite", "Not Favourite"),
        ("video", "Video"),
        ("raw", "RAW Media"),
        ("still photo", "Still Photo"),
    ];

    div()
        .id(ElementId::NamedInteger("val-tray".into(), idx as u64))
        .w_full()
        .p_3()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border())
        .rounded_lg()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SELECT CONDITION"),
                )
                .child(
                    div()
                        .id(ElementId::NamedInteger("close-val-tray".into(), idx as u64))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .hover(|s| s.text_color(theme::text()))
                        .child(render_icon(Icon::X, px(11.0), theme::text_subtle()))
                        .child("Close")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.album_draft.value_dropdown = None;
                            cx.notify();
                        })),
                ),
        )
        .child(div().flex().flex_row().flex_wrap().gap_1p5().children(
            photo_options.into_iter().map(|(val_key, val_text)| {
                let is_curr = photo_choice_label == val_key;
                div()
                    .id(ElementId::Name(
                        format!("photo-opt-{}-{}", idx, val_key).into(),
                    ))
                    .px_2p5()
                    .py_1()
                    .rounded_md()
                    .bg(if is_curr {
                        theme::accent()
                    } else {
                        theme::bg_raised()
                    })
                    .hover(|s| {
                        s.bg(if is_curr {
                            theme::accent_hover()
                        } else {
                            theme::bg_control_hover()
                        })
                    })
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if is_curr {
                        theme::accent_fg()
                    } else {
                        theme::text()
                    })
                    .child(val_text)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.set_draft_rule_photo_choice(idx, val_key, cx);
                    }))
            }),
        ))
}
