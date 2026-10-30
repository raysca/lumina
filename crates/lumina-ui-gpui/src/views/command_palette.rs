//! Cmd+K command palette overlay.

use gpui::prelude::FluentBuilder;
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
                .bg(theme::bg_raised())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(px(10.0))
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
                        .border_color(theme::border_subtle())
                        .child(render_icon(Icon::Search, px(16.0), theme::text_muted()))
                        .child(
                            div()
                                .flex_1()
                                .text_base()
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
                        .child(
                            theme::section_label("COMMANDS")
                                .px(px(12.0))
                                .pt(px(6.0))
                                .pb(px(4.0)),
                        )
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
                                .h(px(34.0))
                                .px(px(12.0))
                                .rounded(px(6.0))
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_center()
                                .gap_3()
                                .map(|row| {
                                    // The active row is the ↵ target, so it
                                    // gets the full accent fill.
                                    if is_selected {
                                        row.bg(theme::accent())
                                    } else {
                                        row.hover(|s| s.bg(theme::bg_control_hover()))
                                    }
                                })
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(if is_selected {
                                            theme::accent_fg()
                                        } else {
                                            theme::text()
                                        })
                                        .child(command.title()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_family(theme::FONT_MONO)
                                        .text_color(if is_selected {
                                            theme::accent_fg()
                                        } else {
                                            theme::text_subtle()
                                        })
                                        .child(command.shortcut),
                                )
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    this.execute_command(id, cx);
                                }))
                        })),
                )
                .child(
                    div()
                        .h(px(32.0))
                        .flex_shrink_0()
                        .px(px(18.0))
                        .flex()
                        .items_center()
                        .border_t_1()
                        .border_color(theme::border_subtle())
                        .bg(theme::bg_pane())
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("↑ ↓ navigate · ↵ run · Esc close"),
                ),
        )
        .into_any_element()
}
