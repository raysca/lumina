//! Toolbar sort field/direction popover.

use gpui::*;
use lumina_core::{SortDirection, SortField};

use crate::app::LuminaApp;
use crate::theme;

/// The dismiss backdrop behind the sort card. The card itself hangs from
/// the header's Sort button (`sort_menu_card`), painted above this.
pub fn render_sort_menu_overlay(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.sort_menu_open {
        return Empty.into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_sort_menu(cx);
            }),
        )
        .into_any_element()
}

/// The sort field/direction card, 210 wide.
pub fn sort_menu_card(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Div {
    let current = app.sort;
    let fields: [(SortField, &'static str); 4] = [
        (SortField::DateTaken, "sort-field-date"),
        (SortField::FileName, "sort-field-name"),
        (SortField::FileSize, "sort-field-size"),
        (SortField::Rating, "sort-field-rating"),
    ];
    div()
        .debug_selector(|| "sort-menu".to_string())
        .w(px(210.0))
        .p_2()
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .occlude()
        .flex()
        .flex_col()
        .gap_1()
        .child(theme::section_label("SORT BY").px_2().py_1())
        .children(fields.into_iter().map(|(field, selector)| {
            let label = crate::app::sort::sort_field_name(field);
            let selected = current.field == field;
            div()
                .id(selector)
                .debug_selector(move || selector.to_string())
                .px_2()
                .py_1p5()
                .rounded(theme::RADIUS_MD)
                .cursor_pointer()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .text_xs()
                .text_color(if selected {
                    theme::accent()
                } else {
                    theme::t1()
                })
                .hover(|s| s.bg(theme::bg_3()))
                .child(label)
                .child(if selected { "✓" } else { "" })
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.set_sort_field(field, cx);
                }))
        }))
        .child(div().h(px(1.0)).bg(theme::line()).my_1())
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
                        .rounded(theme::RADIUS_MD)
                        .cursor_pointer()
                        .text_xs()
                        .text_center()
                        .text_color(if current.direction == SortDirection::Ascending {
                            theme::accent()
                        } else {
                            theme::t2()
                        })
                        .bg(if current.direction == SortDirection::Ascending {
                            theme::bg_3()
                        } else {
                            theme::bg_2()
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
                        .rounded(theme::RADIUS_MD)
                        .cursor_pointer()
                        .text_xs()
                        .text_center()
                        .text_color(if current.direction == SortDirection::Descending {
                            theme::accent()
                        } else {
                            theme::t2()
                        })
                        .bg(if current.direction == SortDirection::Descending {
                            theme::bg_3()
                        } else {
                            theme::bg_2()
                        })
                        .child("Descending ↓")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.set_sort_direction(SortDirection::Descending, cx);
                        })),
                ),
        )
}
