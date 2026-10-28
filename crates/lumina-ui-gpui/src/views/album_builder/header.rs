use gpui::*;
use lumina_core::MatchMode;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_title_row(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(34.0))
                .h(px(34.0))
                .rounded_lg()
                .bg(theme::bg_control())
                .border_1()
                .border_color(theme::border_strong())
                .flex()
                .justify_center()
                .items_center()
                .text_color(theme::accent())
                .child(render_icon(Icon::Sliders, px(16.0), theme::accent())),
        )
        .child(
            div()
                .id("album-title-input-box")
                .flex_1()
                .h(px(36.0))
                .px_3()
                .bg(theme::bg_viewer())
                .border_1()
                .border_color(if app.album_draft.title_focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .rounded_lg()
                .cursor_text()
                .flex()
                .flex_row()
                .items_center()
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.focus_draft_title(cx);
                }))
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text())
                        .child(if app.album_draft.title_focused {
                            if app.album_draft.album_title.is_empty() {
                                "|".to_string()
                            } else {
                                format!("{}|", app.album_draft.album_title)
                            }
                        } else if app.album_draft.album_title.is_empty() {
                            "Untitled Smart Album".to_string()
                        } else {
                            app.album_draft.album_title.clone()
                        }),
                ),
        )
}

pub(super) fn render_presets_row(cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_subtle())
                .mr_1()
                .child("PRESETS:"),
        )
        .child(render_template_pill(
            "Fast Apertures",
            "Fast Apertures",
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.apply_template_fast_apertures(cx);
            }),
        ))
        .child(render_template_pill(
            "High ISO Shots",
            "High ISO Shots",
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.apply_template_high_iso(cx);
            }),
        ))
        .child(render_template_pill(
            "Untagged Photos",
            "Untagged Photos",
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.apply_template_untagged(cx);
            }),
        ))
        .child(render_template_pill(
            "Tagged Media",
            "Tagged Media",
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.apply_template_tagged(cx);
            }),
        ))
        .child(render_template_pill(
            "Sony Favorites",
            "Sony Favorites",
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.apply_template_sony_favorites(cx);
            }),
        ))
}

pub(super) fn render_match_mode_row(
    match_mode: MatchMode,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(theme::text_muted())
                .child("Match"),
        )
        .child(
            div()
                .id("match-mode-toggle-btn")
                .bg(theme::bg_viewer())
                .border_1()
                .border_color(theme::border())
                .rounded_lg()
                .p_0p5()
                .flex()
                .flex_row()
                .gap_0p5()
                .child(
                    div()
                        .id("select-match-all-btn")
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_xs()
                        .bg(if match_mode == MatchMode::All {
                            theme::bg_control()
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if match_mode == MatchMode::All {
                            theme::text()
                        } else {
                            theme::text_subtle()
                        })
                        .font_weight(if match_mode == MatchMode::All {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .child("All (AND)")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.select_draft_logic(MatchMode::All, cx);
                        })),
                )
                .child(
                    div()
                        .id("select-match-any-btn")
                        .px_2p5()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_xs()
                        .bg(if match_mode == MatchMode::Any {
                            theme::bg_control()
                        } else {
                            rgba(0x00000000)
                        })
                        .text_color(if match_mode == MatchMode::Any {
                            theme::text()
                        } else {
                            theme::text_subtle()
                        })
                        .font_weight(if match_mode == MatchMode::Any {
                            FontWeight::MEDIUM
                        } else {
                            FontWeight::NORMAL
                        })
                        .child("Any (OR)")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.select_draft_logic(MatchMode::Any, cx);
                        })),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme::text_muted())
                .child("of the following conditions"),
        )
}

fn render_template_pill(
    name: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(format!("tmpl-{}", name).into()))
        .px_3()
        .py_1()
        .bg(theme::bg_raised())
        .hover(|s| s.bg(theme::bg_control_hover()))
        .border_1()
        .border_color(theme::border())
        .rounded_full()
        .cursor_pointer()
        .text_xs()
        .text_color(theme::text())
        .child(label)
        .on_click(on_click)
}
