//! Cmd+K command palette overlay.

use crate::app::text_input::TextTarget;
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::command_palette::PaletteRow;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

pub fn render_command_palette(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.command_palette_open {
        return Empty.into_any_element();
    }
    let rows = app.palette_rows();
    let selected = app.command_selected.min(rows.len().saturating_sub(1));
    let result_count = rows.len();
    // Each group opens with its heading (the scroll math in
    // move_command_selection counts these).
    let mut list: Vec<AnyElement> = Vec::new();
    let mut group = None;
    for (index, row) in rows.into_iter().enumerate() {
        let heading = row.group();
        if group != Some(heading) {
            group = Some(heading);
            list.push(
                theme::section_label(heading)
                    .px(px(12.0))
                    .pt(px(6.0))
                    .pb(px(4.0))
                    .into_any_element(),
            );
        }
        list.push(render_row(index, index == selected, row, cx));
    }
    let query = app.command_query.clone();

    div()
        .id("command-palette-overlay")
        .debug_selector(|| "command-palette-overlay".to_string())
        .absolute()
        .inset_0()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_start()
        .pt(px(110.0))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.close_command_palette(cx);
            }),
        )
        .child(
            div()
                .id("command-palette")
                .debug_selector(|| "command-palette".to_string())
                .occlude()
                .w(px(640.0))
                .max_h(px(480.0))
                .bg(theme::bg_2())
                .border_1()
                .border_color(theme::line_2())
                .rounded(theme::RADIUS_LG)
                .overflow_hidden()
                .shadow_2xl()
                .flex()
                .flex_col()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .id("command-input")
                        .debug_selector(|| "command-input".to_string())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .h(px(52.0))
                        .flex_shrink_0()
                        .px(px(18.0))
                        .border_b_1()
                        .border_color(theme::line())
                        .child(render_icon(Icon::Search, px(16.0), theme::t2()))
                        .child(
                            div()
                                .flex_1()
                                .text_base()
                                .text_color(if query.is_empty() {
                                    theme::t2()
                                } else {
                                    theme::t1()
                                })
                                .children(app.field_input_for(TextTarget::Palette)),
                        )
                        .child(theme::kbd("esc")),
                )
                .child(
                    div()
                        .id("command-list")
                        .debug_selector(|| "command-list".to_string())
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .track_scroll(&app.command_scroll)
                        .p(px(6.0))
                        .flex()
                        .flex_col()
                        .children(list.is_empty().then(|| {
                            div()
                                .px_3()
                                .py_4()
                                .text_xs()
                                .text_color(theme::t2())
                                .child("No matches")
                        }))
                        .children(list),
                )
                .child(
                    div()
                        .id("command-footer")
                        .debug_selector(|| "command-footer".to_string())
                        .h(px(32.0))
                        .flex_shrink_0()
                        .px(px(18.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(theme::line())
                        .bg(theme::bg_1())
                        .text_xs()
                        .text_color(theme::t2())
                        .child(theme::kbd("↑"))
                        .child(theme::kbd("↓"))
                        .child("move")
                        .child(theme::kbd("↵").ml_2())
                        .child("run")
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("command-result-count")
                                .debug_selector(|| "command-result-count".to_string())
                                .child(
                                    crate::i18n::t(&crate::i18n::plural_key(
                                        "search.results",
                                        result_count,
                                    ))
                                    .replace("{n}", &result_count.to_string()),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

/// 34px row: 16px icon, title, shortcut; the active row is the ↵ target.
fn render_row(
    index: usize,
    is_selected: bool,
    row: PaletteRow,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let fg = if is_selected {
        theme::on_accent()
    } else {
        theme::t1()
    };
    let selector = row.selector;
    let target = row.target;
    div()
        .id(("command-row", index as u64))
        .debug_selector(move || selector.clone())
        .h(px(34.0))
        .flex_shrink_0()
        .px(px(12.0))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .map(|r| {
            if is_selected {
                r.bg(theme::accent())
            } else {
                r.hover(|s| s.bg(theme::bg_3()))
            }
        })
        .child(render_icon(
            row.icon,
            px(16.0),
            if is_selected {
                theme::on_accent()
            } else {
                theme::t2()
            },
        ))
        .child(div().flex_1().text_sm().text_color(fg).child(row.title))
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if is_selected {
                    theme::on_accent()
                } else {
                    theme::t2()
                })
                .child(row.shortcut),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.run_palette_target(target.clone(), cx);
        }))
        .into_any_element()
}
