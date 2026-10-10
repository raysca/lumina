use gpui::*;
use lumina_core::{MediaItem, MediaType};
use std::sync::Arc;

pub struct MediaViewer;

impl MediaViewer {
    pub fn render_overlay(
        item: &MediaItem,
        preview_image: Option<Arc<RenderImage>>,
        on_close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let path = item.path.clone();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Media File")
            .to_string();

        let size_str = format_bytes(item.file_size);
        let type_label = match item.media_type {
            MediaType::Image => "Photo".to_string(),
            MediaType::Raw => "RAW Photo".to_string(),
            MediaType::Video => {
                if item.duration_secs > 0.0 {
                    format!("Video ({:.1}s)", item.duration_secs)
                } else {
                    "Video".to_string()
                }
            }
        };

        div()
            .id("viewer-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x000000ea))
            .flex()
            .flex_col()
            .justify_between()
            .items_center()
            .p_6()
            // Top Bar
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .text_color(rgb(0xffffff))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).child(file_name))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xa1a1aa))
                                    .child(path.to_string_lossy().to_string()),
                            ),
                    )
                    .child(
                        div()
                            .id("close-viewer-btn")
                            .px_3()
                            .py_1p5()
                            .bg(rgb(0x27272a))
                            .hover(|s| s.bg(rgb(0x3f3f46)))
                            .rounded_md()
                            .cursor_pointer()
                            .child("✕ Close")
                            .on_click(on_close),
                    ),
            )
            // Center Image / Video Preview
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .flex()
                    .justify_center()
                    .items_center()
                    .my_4()
                    .child(if let Some(render_img) = preview_image {
                        div()
                            .max_w(px(1000.0))
                            .max_h(px(700.0))
                            .rounded_lg()
                            .overflow_hidden()
                            .shadow_2xl()
                            .child(img(render_img).size_full().object_fit(ObjectFit::Contain))
                    } else {
                        div()
                            .w(px(400.0))
                            .h(px(400.0))
                            .bg(rgb(0x18181b))
                            .rounded_lg()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_color(rgb(0x71717a))
                            .child("Loading full preview...")
                    }),
            )
            // Bottom Info & Actions Bar
            .child(
                div()
                    .w_full()
                    .max_w(px(800.0))
                    .px_6()
                    .py_3()
                    .bg(rgb(0x18181b))
                    .rounded_lg()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .text_color(rgb(0xffffff))
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .items_center()
                            .text_sm()
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .bg(rgb(0x27272a))
                                    .rounded_sm()
                                    .child(type_label),
                            )
                            .child(div().child(size_str))
                            .child(if let Some(ref m) = item.model {
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .bg(rgb(0x1e1e24))
                                    .rounded_sm()
                                    .text_color(rgb(0xd4d4d8))
                                    .child(format!("📷 {}", m))
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            })
                            .child(if let Some(ref c) = item.city {
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .bg(rgb(0x1e293b))
                                    .rounded_sm()
                                    .text_color(rgb(0x38bdf8))
                                    .child(format!("📍 {}", c))
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    )
                    .child(
                        div()
                            .id("reveal-finder-btn")
                            .px_3()
                            .py_1()
                            .bg(rgb(0x3b82f6))
                            .hover(|s| s.bg(rgb(0x2563eb)))
                            .rounded_md()
                            .cursor_pointer()
                            .text_sm()
                            .child("Reveal in Finder")
                            .on_click(move |_event, _window, _cx| {
                                let _ = std::process::Command::new("open")
                                    .arg("-R")
                                    .arg(&path)
                                    .spawn();
                            }),
                    ),
            )
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
