use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::types::{format_bytes, month_name, LayoutMode};
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

pub struct ViewerTopBar;

impl ViewerTopBar {
    pub fn render(
        this: &LuminaApp,
        item: &MediaItem,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let path = item.path.clone();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&item.file_name)
            .to_string();

        let is_video = item.media_type == MediaType::Video;
        let is_inspector_open = this.inspector_open;

        let month_str = month_name(item.month);
        let year_num = if item.year > 0 {
            item.year
        } else {
            this.active_year.unwrap_or(2022)
        };
        let date_prefix = format!("{} {}", month_str, year_num);

        let codec_label = if is_video {
            "H.264"
        } else {
            path.extension().and_then(|e| e.to_str()).unwrap_or("HEIC")
        };

        let fps_label = if is_video { " · 30 fps" } else { "" };
        let width = if item.width > 0 {
            item.width
        } else if is_video {
            1920
        } else {
            4032
        };
        let height = if item.height > 0 {
            item.height
        } else if is_video {
            1080
        } else {
            3024
        };
        let size_str = format_bytes(item.file_size);
        let is_fav = item.is_favorite;

        div()
            .w_full()
            .h(px(52.0))
            .px_4()
            .bg(theme::bg_app())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .text_xs()
            .text_color(theme::text())
            // Left Section: Traffic Light Offset + Segmented Mode Switcher
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    // Traffic lights spacer when windowed
                    .child(div().w(px(64.0)))
                    // Segmented Switcher: [ Browser | Split | Viewer ]
                    .child(
                        theme::segmented("layout-mode-switcher")
                            .child(
                                theme::segmented_item(
                                    "mode-browser",
                                    this.layout_mode == LayoutMode::Browser
                                        && !this.is_fullscreen_viewer,
                                )
                                .child("Browser")
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.close_viewer(cx);
                                        this.layout_mode = LayoutMode::Browser;
                                        this.save_config();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                theme::segmented_item(
                                    "mode-split",
                                    this.layout_mode == LayoutMode::Split
                                        && !this.is_fullscreen_viewer,
                                )
                                .child("Split")
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.close_viewer(cx);
                                        this.layout_mode = LayoutMode::Split;
                                        this.save_config();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                theme::segmented_item(
                                    "mode-viewer",
                                    this.layout_mode == LayoutMode::Viewer
                                        || this.is_fullscreen_viewer,
                                )
                                .child("Viewer")
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.layout_mode = LayoutMode::Viewer;
                                        this.is_fullscreen_viewer = true;
                                        this.save_config();
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            // Center Section: Breadcrumb & Specs Line
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(div().text_color(theme::text_muted()).child(date_prefix))
                    .child(div().text_color(theme::text_disabled()).child(render_icon(
                        Icon::ChevronRight,
                        px(10.0),
                        theme::text_disabled(),
                    )))
                    .child(
                        div()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text())
                            .child(file_name),
                    )
                    .child(div().ml_2().text_color(theme::text_subtle()).child(format!(
                        "{} × {} · {}{} · {}",
                        width, height, codec_label, fps_label, size_str
                    ))),
            )
            // Right Section: Favorite, Info, QuickTime, Reveal in Finder
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    // Favorite Heart Button
                    .child(
                        div()
                            .id("viewer-fav-btn")
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .cursor_pointer()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_sm()
                            .text_color(if is_fav {
                                theme::rating()
                            } else {
                                theme::text_muted()
                            })
                            .child(render_icon(
                                if is_fav {
                                    Icon::HeartFilled
                                } else {
                                    Icon::Heart
                                },
                                px(14.0),
                                if is_fav {
                                    theme::rating()
                                } else {
                                    theme::text_muted()
                                },
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.toggle_favorite_selected(cx);
                            })),
                    )
                    // Info Button (with ⌘I label)
                    .child(
                        div()
                            .id("viewer-info-btn")
                            .px_2p5()
                            .py_1()
                            .bg(if is_inspector_open {
                                theme::selection_bg()
                            } else {
                                theme::bg_control()
                            })
                            .border_1()
                            .border_color(if is_inspector_open {
                                theme::accent()
                            } else {
                                theme::border()
                            })
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(if is_inspector_open {
                                        theme::accent()
                                    } else {
                                        theme::text()
                                    })
                                    .child("Info"),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::text_subtle())
                                    .child("⌘I"),
                            )
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.toggle_inspector(cx);
                            })),
                    )
                    // Open in QuickTime / Preview (with O label)
                    .child({
                        div()
                            .id("viewer-quicktime-btn")
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(if is_video {
                                        "Open in QuickTime"
                                    } else {
                                        "Open in Preview"
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::text_subtle())
                                    .child("O"),
                            )
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, _cx| {
                                this.open_selected_in_default_app();
                            }))
                    })
                    // Reveal in Finder Button
                    .child({
                        let p_clone = path.clone();
                        div()
                            .id("viewer-finder-btn")
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .border_1()
                            .border_color(theme::border())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            .child("Reveal in Finder")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, _cx| {
                                this.reveal_item_in_finder(&p_clone);
                            }))
                    }),
            )
    }
}
