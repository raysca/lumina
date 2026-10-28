//! Cmd+K command palette overlay.

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

pub fn render_command_palette(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.command_palette_open {
        return Empty.into_any_element();
    }
    let commands = app.filtered_commands();
    let selected = app.command_selected.min(commands.len().saturating_sub(1));
    let query = app.command_query.clone();

    div()
        .id("command-palette-overlay")
        .debug_selector(|| "command-palette-overlay".to_string())
        .absolute()
        .inset_0()
        .bg(theme::scrim())
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
                .w(px(560.0))
                .max_h(px(470.0))
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border())
                .rounded_xl()
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
                        .gap_2()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(theme::border_subtle())
                        .child(render_icon(Icon::Search, px(14.0), theme::text_muted()))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(if query.is_empty() {
                                    theme::text_subtle()
                                } else {
                                    theme::text()
                                })
                                .child(if query.is_empty() {
                                    "Type a command…|".to_string()
                                } else {
                                    format!("{}|", query)
                                }),
                        )
                        .child(theme::kbd("⌘K")),
                )
                .child(
                    div()
                        .id("command-list")
                        .max_h(px(340.0))
                        .overflow_y_scroll()
                        .p_2()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .children(commands.is_empty().then(|| {
                            div()
                                .px_3()
                                .py_4()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child("No matching commands")
                        }))
                        .children(commands.iter().enumerate().map(|(index, command)| {
                            let is_selected = index == selected;
                            let id = command.id;
                            let debug_key = command.key;
                            div()
                                .id(("command-row", index as u64))
                                .debug_selector(move || format!("command-item-{debug_key}"))
                                .px_3()
                                .py_2()
                                .rounded_md()
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .gap_3()
                                .bg(if is_selected {
                                    theme::selection_bg()
                                } else {
                                    theme::bg_pane()
                                })
                                .hover(|s| s.bg(theme::selection_bg_hover()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(if is_selected {
                                            theme::accent()
                                        } else {
                                            theme::text()
                                        })
                                        .child(command.title()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(theme::FONT_MONO)
                                        .text_color(theme::text_subtle())
                                        .child(command.shortcut),
                                )
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.execute_command(id, cx);
                                }))
                        })),
                )
                .child(
                    div()
                        .px_4()
                        .py_2()
                        .border_t_1()
                        .border_color(theme::border_subtle())
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("↑ ↓ navigate · ↵ run · Esc close"),
                ),
        )
        .into_any_element()
}
