//! Toolbar sort field/direction popover.

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::app::LuminaApp;
use crate::theme;

pub fn render_sort_menu_overlay(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.sort_menu_open {
        return Empty.into_any_element();
    }
    let current = app.sort;
    let fields: [(&str, SortField, &'static str); 4] = [
        ("Date taken", SortField::DateTaken, "sort-field-date"),
        ("Name", SortField::FileName, "sort-field-name"),
        ("File size", SortField::FileSize, "sort-field-size"),
        ("Rating", SortField::Rating, "sort-field-rating"),
    ];
    div()
        .absolute()
        .inset_0()
        .child(div().absolute().inset_0().on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_sort_menu(cx);
            }),
        ))
        .child(
            div()
                .absolute()
                .top(px(58.0))
                .right(px(16.0))
                .w(px(210.0))
                .p_2()
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border())
                .rounded_lg()
                .shadow_2xl()
                .occlude()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SORT BY"),
                )
                .children(fields.into_iter().map(|(label, field, selector)| {
                    let selected = current.field == field;
                    div()
                        .id(selector)
                        .debug_selector(move || selector.to_string())
                        .px_2()
                        .py_1p5()
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .text_xs()
                        .text_color(if selected {
                            theme::accent()
                        } else {
                            theme::text()
                        })
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .child(label.to_string())
                        .child(if selected { "✓" } else { "" })
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.set_sort_field(field, cx);
                        }))
                }))
                .child(div().h(px(1.0)).bg(theme::border_subtle()).my_1())
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_1()
                        .px_1()
                        .child(
                            div()
                                .id("sort-direction-asc")
                                .debug_selector(|| "sort-direction-asc".to_string())
                                .flex_1()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .text_center()
                                .text_color(if current.direction == SortDirection::Ascending {
                                    theme::accent()
                                } else {
                                    theme::text_muted()
                                })
                                .bg(if current.direction == SortDirection::Ascending {
                                    theme::selection_bg()
                                } else {
                                    theme::bg_control()
                                })
                                .child("Ascending ↑")
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_sort_direction(SortDirection::Ascending, cx);
                                })),
                        )
                        .child(
                            div()
                                .id("sort-direction-desc")
                                .debug_selector(|| "sort-direction-desc".to_string())
                                .flex_1()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .text_center()
                                .text_color(if current.direction == SortDirection::Descending {
                                    theme::accent()
                                } else {
                                    theme::text_muted()
                                })
                                .bg(if current.direction == SortDirection::Descending {
                                    theme::selection_bg()
                                } else {
                                    theme::bg_control()
                                })
                                .child("Descending ↓")
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_sort_direction(SortDirection::Descending, cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}
