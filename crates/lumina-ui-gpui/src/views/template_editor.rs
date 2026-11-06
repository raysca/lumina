//! Metadata Templates sheet: the saved templates down the side, one
//! template's name, rights fields and keywords beside them, Save / Cancel /
//! Delete. Keys are in `keymap/modals.rs`.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::templates::TemplateField;
use crate::app::text_input::TextTarget;
use crate::i18n::t;
use crate::theme;
use crate::LuminaApp;

pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(editor) = app.templates.editor.as_ref() else {
        return Empty.into_any_element();
    };

    let button = |id: &'static str, label: String| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .role(Role::Button)
            .aria_label(label.clone())
            .h(px(30.0))
            .px_3()
            .rounded(theme::RADIUS_MD)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .child(label)
    };

    // The list: every template, then "New Template".
    let mut rows: Vec<AnyElement> = app
        .config
        .templates
        .iter()
        .enumerate()
        .map(|(i, template)| {
            list_row(
                format!("template-row-{i}"),
                template.name.clone(),
                editor.editing == Some(i),
                Some(i),
                cx,
            )
        })
        .collect();
    rows.push(list_row(
        "template-row-new".to_string(),
        t("template.new"),
        editor.editing.is_none(),
        None,
        cx,
    ));

    let fields = TemplateField::ALL.map(|field| {
        let focused = editor.focus == field;
        let value = editor.value(field).to_string();
        let label = field.label();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(label.clone()),
            )
            .child(
                theme::field(SharedString::from(format!(
                    "template-field-{}",
                    field.slug()
                )))
                .debug_selector(move || format!("template-field-{}", field.slug()))
                // The focused field's input reports itself.
                .when(!focused, |f| f.role(Role::TextInput).aria_label(label))
                .border_color(if focused {
                    theme::focus_ring()
                } else {
                    theme::border()
                })
                .overflow_hidden()
                .whitespace_nowrap()
                .child(match app.field_input_for(TextTarget::Template(field)) {
                    Some(input) => input.into_any_element(),
                    None => div()
                        .text_color(theme::text_muted())
                        .child(value)
                        .into_any_element(),
                })
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.focus_template_field(field, cx);
                })),
            )
            .into_any_element()
    });

    let editing_existing = editor.editing.is_some();
    let error = editor.error.clone();

    div()
        .id("template-editor-backdrop")
        .debug_selector(|| "template-editor-backdrop".to_string())
        .absolute()
        .inset_0()
        .occlude()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_center()
        .p_8()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.dismiss_template_editor(cx);
            }),
        )
        .child(
            div()
                .id("template-editor")
                .debug_selector(|| "template-editor".to_string())
                .role(Role::Dialog)
                .aria_label(t("template.title"))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .w(px(640.0))
                .p_5()
                .bg(theme::bg_dialog())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(theme::RADIUS_LG)
                .shadow_2xl()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child(t("template.title")),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(t("template.intro")),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_4()
                        .child(
                            div()
                                .id("template-list")
                                .debug_selector(|| "template-list".to_string())
                                .w(px(180.0))
                                .max_h(px(380.0))
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .children(rows),
                        )
                        .child(div().flex_1().flex().flex_col().gap_2().children(fields)),
                )
                .children(error.map(|error| {
                    div()
                        .debug_selector(|| "template-error".to_string())
                        .text_xs()
                        .text_color(theme::danger_text())
                        .child(error)
                }))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .when(editing_existing, |row| {
                            row.child(
                                button("template-delete", t("template.delete"))
                                    .bg(theme::bg_control())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .text_color(theme::danger_text())
                                    .tooltip(theme::tooltip_label(t("template.delete_tooltip")))
                                    .child(theme::kbd("⇧⌘⌫"))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.delete_template_in_editor(cx);
                                    })),
                            )
                        })
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(t("template.keys_hint")),
                        )
                        .child(
                            button("template-cancel", t("template.cancel"))
                                .bg(theme::bg_control())
                                .hover(|s| s.bg(theme::bg_control_hover()))
                                .text_color(theme::text())
                                .child(theme::kbd("Esc"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_template_editor(cx);
                                })),
                        )
                        .child(
                            button("template-save", t("template.save"))
                                .bg(theme::accent())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::accent_fg())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.save_template_editor(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}

/// One list entry; clicking shows that template (`None`: a new one).
fn list_row(
    selector: String,
    title: String,
    selected: bool,
    editing: Option<usize>,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(title.clone())
        .h(px(28.0))
        .flex_shrink_0()
        .px_2()
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .flex()
        .items_center()
        .text_xs()
        .truncate()
        .map(|row| {
            if selected {
                row.bg(theme::accent()).text_color(theme::accent_fg())
            } else {
                row.text_color(theme::text())
                    .hover(|s| s.bg(theme::bg_control_hover()))
            }
        })
        .child(title)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.pick_template_in_editor(editing, cx);
        }))
        .into_any_element()
}
