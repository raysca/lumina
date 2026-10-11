use gpui::*;
use lumina_core::MediaType;
use crate::LuminaApp;

pub struct InspectorView;

impl InspectorView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let item = match this.selected_item.as_ref() {
            Some(it) => it,
            None => return Empty.into_any_element(),
        };

        let path = item.path.clone();
        let file_name = item.file_name.clone();
        let size_str = format_bytes(item.file_size);
        let preview_image = this.viewer_image.clone();

        let type_label = match item.media_type {
            MediaType::Image => "Photo",
            MediaType::Raw => "RAW Photo",
            MediaType::Video => "Video",
        };

        let is_fav = item.is_favorite;

        div()
            .w(px(310.0))
            .h_full()
            .bg(rgb(0x121215))
            .border_l_1()
            .border_color(rgb(0x27272a))
            .flex()
            .flex_col()
            .text_color(rgb(0xffffff))
            // Header
            .child(
                div()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(0x27272a))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(div().text_sm().child("ℹ"))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xa1a1aa))
                                    .child("INFO & METADATA"),
                            ),
                    )
                    .child(
                        div()
                            .id("close-inspector-btn")
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded_md()
                            .hover(|s| s.bg(rgb(0x27272a)))
                            .flex()
                            .justify_center()
                            .items_center()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(0x71717a))
                            .child("✕")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.inspector_open = false;
                                cx.notify();
                            })),
                    ),
            )
            // Scrollable metadata body
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // Thumbnail Preview Card
                    .child(
                        div()
                            .w_full()
                            .h(px(160.0))
                            .rounded_lg()
                            .bg(rgb(0x18181b))
                            .overflow_hidden()
                            .flex()
                            .justify_center()
                            .items_center()
                            .child(if let Some(img_render) = preview_image {
                                div()
                                    .size_full()
                                    .child(img(img_render).size_full().object_fit(ObjectFit::Cover))
                                    .into_any_element()
                            } else {
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x71717a))
                                    .child("Preview Loading...")
                                    .into_any_element()
                            }),
                    )
                    // File Name and Favorite Toggle
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xffffff))
                                    .child(file_name),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("inspector-fav-btn")
                                            .flex_1()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(if is_fav { rgb(0xd97706) } else { rgb(0x27272a) })
                                            .hover(move |s| {
                                                if is_fav {
                                                    s.bg(rgb(0xb45309))
                                                } else {
                                                    s.bg(rgb(0x3f3f46))
                                                }
                                            })
                                            .cursor_pointer()
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .gap_1p5()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(if is_fav { "★ Favorited" } else { "☆ Favorite" })
                                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                                this.toggle_favorite_selected(cx);
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("inspector-reveal-btn")
                                            .flex_1()
                                            .py_1p5()
                                            .rounded_md()
                                            .bg(rgb(0x27272a))
                                            .hover(|s| s.bg(rgb(0x3f3f46)))
                                            .cursor_pointer()
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Reveal ↗")
                                            .on_click(move |_e, _w, _cx| {
                                                let _ = std::process::Command::new("open")
                                                    .arg("-R")
                                                    .arg(&path)
                                                    .spawn();
                                            }),
                                    ),
                            ),
                    )
                    // Camera & Optics Section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(0x18181b))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x71717a))
                                    .child("CAMERA & OPTICS"),
                            )
                            .child(render_meta_row(
                                "Camera",
                                item.model.as_deref().or(item.make.as_deref()).unwrap_or("Unknown Camera"),
                            ))
                            .child(if let Some(ref l) = item.lens {
                                render_meta_row("Lens", l).into_any_element()
                            } else {
                                Empty.into_any_element()
                            })
                            .child(if item.focal_length.is_some() || item.f_number.is_some() || item.exposure_time.is_some() || item.iso.is_some() {
                                let mut settings = Vec::new();
                                if let Some(fl) = item.focal_length {
                                    settings.push(format!("{:.0}mm", fl));
                                }
                                if let Some(f) = item.f_number {
                                    settings.push(format!("f/{:.1}", f));
                                }
                                if let Some(ref exp) = item.exposure_time {
                                    settings.push(exp.clone());
                                }
                                if let Some(iso) = item.iso {
                                    settings.push(format!("ISO {}", iso));
                                }
                                render_meta_row("Settings", &settings.join(" • ")).into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    )
                    // Location Section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(0x18181b))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x71717a))
                                    .child("LOCATION & GEOGRAPHY"),
                            )
                            .child(if let Some(ref city) = item.city {
                                let country = item.country_code.as_deref().unwrap_or("");
                                let region = item.admin1_region.as_deref().unwrap_or("");
                                let full_loc = format!("{}, {}, {}", city, region, country);
                                render_meta_row("Place", &full_loc).into_any_element()
                            } else {
                                render_meta_row("Place", "No GPS metadata").into_any_element()
                            })
                            .child(if let (Some(lat), Some(lon)) = (item.latitude, item.longitude) {
                                let coords = format!("{:.4}°, {:.4}°", lat, lon);
                                render_meta_row("GPS", &coords).into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    )
                    // File Attributes Section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(0x18181b))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x71717a))
                                    .child("FILE ATTRIBUTES"),
                            )
                            .child(render_meta_row("Type", type_label))
                            .child(render_meta_row("Dimensions", &format!("{} × {}", item.width, item.height)))
                            .child(render_meta_row("File Size", &size_str))
                            .child(render_meta_row(
                                "Date Taken",
                                &format!("{:04}-{:02}-{:02}", item.year, item.month, item.day),
                            ))
                            .child(if item.duration_secs > 0.0 {
                                render_meta_row("Duration", &format!("{:.1}s", item.duration_secs)).into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    ),
            )
            .into_any_element()
    }
}

fn render_meta_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_start()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x71717a))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xd4d4d8))
                .text_right()
                .child(value.to_string()),
        )
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
