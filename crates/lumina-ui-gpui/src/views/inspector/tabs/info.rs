use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::tabs::trash::render_trash_lifecycle;
use crate::LuminaApp;

#[allow(clippy::too_many_arguments)]
pub fn render_info_tab(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    path: &std::path::Path,
    file_name: &str,
    size_str: &str,
    is_trash: bool,
    rating: u8,
    display_title: &str,
    caption: Option<&str>,
    caption_input_active: bool,
    caption_input_buffer: &str,
    title_input_active: bool,
    title_input_buffer: &str,
) -> AnyElement {
    // Kind string derived from file extension / media type
    let kind_str = if file_name.ends_with(".HEIC") || file_name.ends_with(".heic") {
        "HEIC image"
    } else if file_name.ends_with(".png") || file_name.ends_with(".PNG") {
        "PNG image"
    } else if file_name.ends_with(".MOV")
        || file_name.ends_with(".mov")
        || file_name.ends_with(".mp4")
    {
        "QuickTime movie"
    } else if item.media_type == MediaType::Raw {
        "RAW image"
    } else {
        "JPEG image"
    };

    // Dimensions with megapixels
    let dimensions_str = if item.width > 0 && item.height > 0 {
        let mp = (item.width as f64 * item.height as f64) / 1_000_000.0;
        format!("{} × {} ({:.1} MP)", item.width, item.height, mp)
    } else {
        "Unknown".to_string()
    };

    // Formatted dates from real epoch timestamps
    let date_taken_str = item.formatted_timestamp();
    let modified_str = item.formatted_mtime();

    // Colour profile
    let color_profile = if file_name.ends_with(".HEIC") || file_name.ends_with(".heic") {
        "Display P3"
    } else {
        "sRGB IEC61966-2.1"
    };

    let path_str = path.display().to_string();
    let app_caption_start = weak_app.clone();
    let app_title_start = weak_app.clone();

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        // 1. Two-Column Information Attributes Form (matching mockup)
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap_2p5()
                // Title (click to edit)
                .child(render_info_field_row(
                    "Title",
                    div()
                        .id("inspector-title-box")
                        .debug_selector(|| "inspector-title-box".to_string())
                        .w_full()
                        .px_3()
                        .py_1p5()
                        .bg(theme::bg_app())
                        .border_1()
                        .border_color(if title_input_active {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .hover(|s| {
                            if title_input_active {
                                s
                            } else {
                                s.border_color(theme::border_strong())
                            }
                        })
                        .rounded_md()
                        .cursor_text()
                        .text_xs()
                        .text_color(theme::text())
                        .child(if title_input_active {
                            if title_input_buffer.is_empty() {
                                "Type a title... (Enter to save, Esc to cancel)|".to_string()
                            } else {
                                format!("{}|", title_input_buffer)
                            }
                        } else {
                            display_title.to_string()
                        })
                        .on_click(move |_e, _w, cx| {
                            if !title_input_active {
                                if let Some(app) = app_title_start.upgrade() {
                                    app.update(cx, |this, cx| {
                                        this.start_title_input(cx);
                                    });
                                }
                            }
                        }),
                ))
                // Caption (Interactive click-to-edit & SQLite persistence)
                .child(render_info_field_row(
                    "Caption",
                    div()
                        .id("inspector-caption-box")
                        .w_full()
                        .min_h(px(54.0))
                        .px_3()
                        .py_2()
                        .bg(theme::bg_app())
                        .border_1()
                        .border_color(if caption_input_active {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .hover(|s| {
                            if caption_input_active {
                                s
                            } else {
                                s.border_color(theme::border_strong())
                            }
                        })
                        .rounded_md()
                        .cursor_text()
                        .text_xs()
                        .text_color(if caption_input_active || caption.is_some() {
                            theme::text()
                        } else {
                            theme::text_subtle()
                        })
                        .child(if caption_input_active {
                            if caption_input_buffer.is_empty() {
                                "Type caption... (Enter to save, Esc to cancel)|".to_string()
                            } else {
                                format!("{}|", caption_input_buffer)
                            }
                        } else if let Some(cap) = caption {
                            cap.to_string()
                        } else {
                            "Add a caption".to_string()
                        })
                        .on_click(move |_e, _w, cx| {
                            if !caption_input_active {
                                if let Some(app) = app_caption_start.upgrade() {
                                    app.update(cx, |this, cx| {
                                        this.start_caption_input(cx);
                                    });
                                }
                            }
                        }),
                ))
                // Date taken
                .child(render_info_field_row(
                    "Date taken",
                    div().flex().flex_row().items_center().gap_2().child(
                        div()
                            .text_xs()
                            .text_color(theme::text())
                            .child(date_taken_str),
                    ),
                ))
                // Rating (interactive gold star icons)
                .child(render_info_field_row(
                    "Rating",
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .children((1u8..=5).map(|star| {
                            let app_star = weak_app.clone();
                            let is_filled = rating >= star;
                            div()
                                .id(ElementId::NamedInteger(
                                    "inspector-star".into(),
                                    star as u64,
                                ))
                                .cursor_pointer()
                                .text_xs()
                                .text_color(if is_filled {
                                    theme::rating()
                                } else {
                                    theme::text_disabled()
                                })
                                .child(render_icon(
                                    if is_filled {
                                        Icon::StarFilled
                                    } else {
                                        Icon::Star
                                    },
                                    px(12.0),
                                    if is_filled {
                                        theme::rating()
                                    } else {
                                        theme::text_disabled()
                                    },
                                ))
                                .on_click(move |_e, _w, cx| {
                                    if let Some(app) = app_star.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.set_rating_selected(star, cx);
                                        });
                                    }
                                })
                        })),
                ))
                // Kind
                .child(render_info_label_value_row("Kind", kind_str))
                // Dimensions
                .child(render_info_label_value_row("Dimensions", &dimensions_str))
                // File size
                .child(render_info_label_value_row("File size", size_str))
                // Colour profile
                .child(render_info_label_value_row("Colour profile", color_profile))
                // Modified
                .child(render_info_label_value_row("Modified", &modified_str))
                // Path
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_start()
                        .child(
                            div()
                                .w(px(115.0))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Path"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .font_family(theme::FONT_MONO)
                                .text_color(theme::text_muted())
                                .child(path_str),
                        ),
                ),
        )
        // 2. Trash Lifecycle Actions (when viewing Recently Deleted)
        .child(if is_trash {
            render_trash_lifecycle(weak_app)
        } else {
            Empty.into_any_element()
        })
        .into_any_element()
}

fn render_info_field_row(label: &str, content: impl IntoElement) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .w(px(115.0))
                .flex_shrink_0()
                .text_xs()
                .text_color(theme::text_muted())
                .child(label.to_string()),
        )
        .child(div().flex_1().child(content))
}

fn render_info_label_value_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_baseline()
        .child(
            div()
                .w(px(115.0))
                .flex_shrink_0()
                .text_xs()
                .text_color(theme::text_muted())
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(theme::text())
                .child(value.to_string()),
        )
}
