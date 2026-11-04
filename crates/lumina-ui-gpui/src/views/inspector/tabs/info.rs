use crate::app::caret::caret;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::info_details::InfoDetails;
use crate::theme;
use crate::views::inspector::summary::render_stars;
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
    star_pop: Option<(u8, ElementId)>,
    display_title: &str,
    caption: Option<&str>,
    caption_input_active: bool,
    caption_input_buffer: &str,
    title_input_active: bool,
    title_input_buffer: &str,
    date_input_active: bool,
    date_input_buffer: &str,
    details: Option<&InfoDetails>,
) -> AnyElement {
    let kind_str = kind_label(item.media_type, file_name);

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

    // Only what the file states; no guessing from the extension.
    let color_profile = details.and_then(|d| d.file.color_space.clone());
    let created_str = details
        .and_then(|d| d.file.created_epoch_ms)
        .map(format_epoch);
    let added_str = details.and_then(|d| d.added_epoch_ms).map(format_epoch);

    let path_str = path.display().to_string();
    let app_caption_start = weak_app.clone();
    let app_title_start = weak_app.clone();
    let app_date_start = weak_app.clone();

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
                // v3: rows on a 160px label grid with 1px dividers.
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
                            theme::focus_ring()
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
                        .rounded(theme::RADIUS_MD)
                        .cursor_text()
                        .text_size(theme::TEXT_BODY)
                        .text_color(theme::text())
                        .child(if title_input_active {
                            if title_input_buffer.is_empty() {
                                format!("Type a title... (Enter to save, Esc to cancel){}", caret())
                            } else {
                                format!("{}{}", title_input_buffer, caret())
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
                            theme::focus_ring()
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
                        .rounded(theme::RADIUS_MD)
                        .cursor_text()
                        .text_size(theme::TEXT_BODY)
                        .text_color(if caption_input_active || caption.is_some() {
                            theme::text()
                        } else {
                            theme::text_subtle()
                        })
                        .child(if caption_input_active {
                            if caption_input_buffer.is_empty() {
                                format!("Type caption... (Enter to save, Esc to cancel){}", caret())
                            } else {
                                format!("{}{}", caption_input_buffer, caret())
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
                // Date taken: text plus "Adjust…", which opens the editor.
                .child(render_info_field_row(
                    "Date taken",
                    if date_input_active {
                        div()
                            .id("inspector-date-box")
                            .debug_selector(|| "inspector-date-box".to_string())
                            .w_full()
                            .px_3()
                            .py_1p5()
                            .bg(theme::bg_app())
                            .border_1()
                            .border_color(theme::focus_ring())
                            .rounded(theme::RADIUS_MD)
                            .text_size(theme::TEXT_BODY)
                            .text_color(theme::text())
                            .child(if date_input_buffer.is_empty() {
                                format!("YYYY-MM-DD HH:MM:SS{}", caret())
                            } else {
                                format!("{}{}", date_input_buffer, caret())
                            })
                            .into_any_element()
                    } else {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child(date_taken_str),
                            )
                            .child(
                                div()
                                    .id("inspector-date-adjust")
                                    .debug_selector(|| "inspector-date-adjust".to_string())
                                    .tooltip(theme::tooltip_label("Adjust the date taken"))
                                    .h(px(22.0))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .rounded(theme::RADIUS_MD)
                                    .border_1()
                                    .border_color(theme::border())
                                    .bg(theme::bg_raised())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child("Adjust…")
                                    .on_click(move |_e, _w, cx| {
                                        if let Some(app) = app_date_start.upgrade() {
                                            app.update(cx, |this, cx| {
                                                this.start_date_input(cx);
                                            });
                                        }
                                    }),
                            )
                            .into_any_element()
                    },
                ))
                // Rating (interactive gold star icons)
                .child(render_info_field_row(
                    "Rating",
                    render_stars(weak_app.clone(), rating, star_pop),
                ))
                // Kind
                .child(render_info_label_value_row("Kind", &kind_str))
                // Dimensions
                .child(render_info_label_value_row("Dimensions", &dimensions_str))
                // File size
                .child(render_info_label_value_row("File size", size_str))
                .children(
                    color_profile
                        .as_deref()
                        .map(|profile| render_info_label_value_row("Color profile", profile)),
                )
                .child(render_info_label_value_row(
                    "Created",
                    created_str.as_deref().unwrap_or("—"),
                ))
                .child(render_info_label_value_row("Modified", &modified_str))
                .child(render_info_label_value_row(
                    "Added to Lumina",
                    added_str.as_deref().unwrap_or("—"),
                ))
                .children(
                    details
                        .and_then(|d| d.source.as_ref())
                        .map(|(url, uploader, _ts)| {
                            let text = match uploader {
                                Some(u) if !u.is_empty() => format!("{u} ({url})"),
                                _ => url.clone(),
                            };
                            render_info_label_value_row("Source", &text)
                        }),
                )
                // Path
                .child(
                    div()
                        .w_full()
                        .min_h(px(ROW_MIN_HEIGHT))
                        .border_b_1()
                        .border_color(theme::border_subtle())
                        .py_2()
                        .flex()
                        .flex_row()
                        .items_start()
                        .child(
                            div()
                                .w(px(LABEL_WIDTH))
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child("Path"),
                        )
                        .child(
                            div()
                                .flex_1()
                                // Long absolute paths are a single unbroken
                                // token; without min_w(0) they widen the row
                                // instead of wrapping (BUG 12).
                                .min_w(px(0.0))
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

const LABEL_WIDTH: f32 = 160.0;
const ROW_MIN_HEIGHT: f32 = 32.0;

fn format_epoch(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format("%e %b %Y, %H:%M").to_string().trim().to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// "JPEG image", "QuickTime movie"… from the extension, which is what the
/// scanner classified the file by.
fn kind_label(media_type: MediaType, file_name: &str) -> String {
    let ext = std::path::Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match (media_type, ext.as_str()) {
        (_, "jpg" | "jpeg") => "JPEG image".into(),
        (_, "heic" | "heif") => "HEIF image".into(),
        (_, "png") => "PNG image".into(),
        (_, "webp") => "WebP image".into(),
        (_, "tif" | "tiff") => "TIFF image".into(),
        (_, "mov") => "QuickTime movie".into(),
        (_, "mp4" | "m4v") => "MPEG-4 movie".into(),
        (MediaType::Raw, ext) => format!("RAW image ({})", ext.to_ascii_uppercase()),
        (MediaType::Video, _) => "Video".into(),
        (MediaType::Image, _) => "Image".into(),
    }
}

fn render_info_field_row(label: &str, content: impl IntoElement) -> impl IntoElement {
    div()
        .w_full()
        .min_h(px(ROW_MIN_HEIGHT))
        .border_b_1()
        .border_color(theme::border_subtle())
        .py_1p5()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .w(px(LABEL_WIDTH))
                .flex_shrink_0()
                .text_xs()
                .text_color(theme::text_muted())
                .child(label.to_string()),
        )
        .child(div().flex_1().min_w(px(0.0)).child(content))
}

/// Label/value row; `info-row-<label>` is its automation selector.
pub(crate) fn render_info_label_value_row(label: &str, value: &str) -> impl IntoElement {
    let selector = format!("info-row-{}", label.to_lowercase().replace(' ', "-"));
    div()
        .debug_selector(move || selector.clone())
        .w_full()
        .min_h(px(ROW_MIN_HEIGHT))
        .border_b_1()
        .border_color(theme::border_subtle())
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .w(px(LABEL_WIDTH))
                .flex_shrink_0()
                .text_xs()
                .text_color(theme::text_muted())
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .text_xs()
                .text_color(theme::text())
                .child(value.to_string()),
        )
}
