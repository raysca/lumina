use std::sync::Arc;

use gpui::*;
use lumina_core::{MediaItem, MediaType};

pub use crate::app::types::format_bytes;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::batch::render_batch_body;
use crate::views::inspector::tabs::camera::render_camera_tab;
use crate::views::inspector::tabs::info::render_info_tab;
use crate::views::inspector::tabs::location::render_location_tab;
use crate::views::inspector::tabs::tags::render_tags_tab;
use crate::{BatchSelectionSummary, LuminaApp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Info,
    Camera,
    Location,
    Tags,
}

impl InspectorTab {
    pub fn config_key(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Camera => "camera",
            Self::Location => "location",
            Self::Tags => "tags",
        }
    }

    pub fn from_config_key(value: Option<&str>) -> Self {
        match value {
            Some("camera") => Self::Camera,
            Some("location") => Self::Location,
            Some("tags") => Self::Tags,
            _ => Self::Info,
        }
    }
}

pub fn render_meta_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_start()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text())
                .text_right()
                .child(value.to_string()),
        )
}

#[allow(clippy::too_many_arguments)]
pub fn render_inspector_body(
    weak_app: WeakEntity<LuminaApp>,
    selected_item: Option<MediaItem>,
    selected_count: usize,
    batch_summary: &BatchSelectionSummary,
    preview_image: Option<Arc<RenderImage>>,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    is_trash: bool,
    tab: InspectorTab,
    rating: u8,
    color_label: Option<u8>,
    caption: Option<&str>,
    caption_input_active: bool,
    caption_input_buffer: &str,
    title_input_active: bool,
    title_input_buffer: &str,
) -> AnyElement {
    if selected_count > 1 {
        return render_batch_body(
            weak_app,
            batch_summary,
            tags,
            library_tag_counts,
            tag_input_active,
            tag_input_buffer,
            is_trash,
            rating,
            color_label,
        );
    }

    let item = match selected_item {
        Some(it) => it,
        None => {
            return div()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .text_sm()
                .text_color(theme::text_subtle())
                .child("No media selected")
                .into_any_element();
        }
    };

    render_single_body(
        weak_app,
        item,
        preview_image,
        tags,
        library_tag_counts,
        tag_input_active,
        tag_input_buffer,
        is_trash,
        tab,
        rating,
        caption,
        caption_input_active,
        caption_input_buffer,
        title_input_active,
        title_input_buffer,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_single_body(
    weak_app: WeakEntity<LuminaApp>,
    item: MediaItem,
    preview_image: Option<Arc<RenderImage>>,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    is_trash: bool,
    tab: InspectorTab,
    rating: u8,
    caption: Option<&str>,
    caption_input_active: bool,
    caption_input_buffer: &str,
    title_input_active: bool,
    title_input_buffer: &str,
) -> AnyElement {
    let path = item.path.clone();
    let file_name = item.file_name.clone();
    let size_str = format_bytes(item.file_size);
    let is_fav = item.is_favorite;

    let type_label = match item.media_type {
        MediaType::Image => "Photo",
        MediaType::Raw => "RAW Photo",
        MediaType::Video => "Video",
    };

    // Prefer the user title, then location, then filename stem.
    let display_title = if let Some(ref title) = item.title {
        title.clone()
    } else if let Some(ref city) = item.city {
        if let Some(ref country) = item.country_code {
            format!("{}, {}", city, country)
        } else {
            city.clone()
        }
    } else {
        std::path::Path::new(&item.file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&item.file_name)
            .to_string()
    };

    let dim_str = if item.width > 0 && item.height > 0 {
        format!("{} × {}", item.width, item.height)
    } else {
        "—".to_string()
    };
    let date_str = item.formatted_timestamp();
    let meta_summary_line = format!("{} · {} · {} · {}", type_label, dim_str, size_str, date_str);

    let app_fav = weak_app.clone();
    let app_reveal = weak_app.clone();
    let app_open_with = weak_app.clone();
    let path_for_reveal = path.clone();
    let path_for_copy = path.clone();

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme::bg_pane())
        .text_color(theme::text())
        .overflow_hidden()
        // 1. Top Hero Media Card (Clean layout matching mockup)
        .child(
            div()
                .w_full()
                .p_4()
                .border_b_1()
                .border_color(theme::border_subtle())
                .flex()
                .flex_row()
                .gap_3p5()
                .items_start()
                // Left: 90x90 Thumbnail
                .child(
                    div()
                        .w(px(90.0))
                        .h(px(90.0))
                        .flex_shrink_0()
                        .rounded_lg()
                        .overflow_hidden()
                        .border_1()
                        .border_color(theme::border_subtle())
                        .bg(theme::bg_app())
                        .child(if let Some(ref img_render) = preview_image {
                            img(img_render.clone())
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .into_any_element()
                        } else {
                            div().size_full().bg(theme::bg_app()).into_any_element()
                        }),
                )
                // Right: Titles & Quick Actions
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        // Title
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child(display_title.clone()),
                        )
                        // Filename
                        .child(
                            div()
                                .text_xs()
                                .font_family(theme::FONT_MONO)
                                .text_color(theme::text_subtle())
                                .child(file_name.clone()),
                        )
                        // Metadata Summary
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_muted())
                                .child(meta_summary_line),
                        )
                        // Quick Action Pill Buttons
                        .child(
                            div()
                                .pt_1p5()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .flex_wrap()
                                // 1. Favorite
                                .child(
                                    div()
                                        .id("info-quick-fav-btn")
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(theme::bg_raised())
                                        .hover(|s| s.bg(theme::bg_control_hover()))
                                        .border_1()
                                        .border_color(theme::border())
                                        .cursor_pointer()
                                        .text_xs()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_1p5()
                                        .text_color(if is_fav {
                                            theme::accent()
                                        } else {
                                            theme::text()
                                        })
                                        .child(render_icon(
                                            if is_fav {
                                                Icon::HeartFilled
                                            } else {
                                                Icon::Heart
                                            },
                                            px(11.0),
                                            if is_fav {
                                                theme::accent()
                                            } else {
                                                theme::text()
                                            },
                                        ))
                                        .child("Favorite")
                                        .on_click(move |_e, _w, cx| {
                                            if let Some(app) = app_fav.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.toggle_favorite_selected(cx);
                                                });
                                            }
                                        }),
                                )
                                // 2. Reveal in Finder
                                .child(
                                    div()
                                        .id("info-quick-reveal-btn")
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(theme::bg_raised())
                                        .hover(|s| s.bg(theme::bg_control_hover()))
                                        .border_1()
                                        .border_color(theme::border())
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::text())
                                        .child("Reveal in Finder")
                                        .on_click(move |_e, _w, cx| {
                                            if let Some(app) = app_reveal.upgrade() {
                                                app.update(cx, |this, _cx| {
                                                    this.reveal_item_in_finder(&path_for_reveal);
                                                });
                                            }
                                        }),
                                )
                                // 3. Copy Path
                                .child(
                                    div()
                                        .id("info-quick-copy-path-btn")
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(theme::bg_raised())
                                        .hover(|s| s.bg(theme::bg_control_hover()))
                                        .border_1()
                                        .border_color(theme::border())
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::text())
                                        .child("Copy Path")
                                        .on_click(move |_e, _w, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                path_for_copy.display().to_string(),
                                            ));
                                        }),
                                )
                                // 4. Open in default application
                                .child(
                                    div()
                                        .id("info-quick-open-with-btn")
                                        .px_2p5()
                                        .py_0p5()
                                        .rounded_full()
                                        .bg(theme::bg_raised())
                                        .hover(|s| s.bg(theme::bg_control_hover()))
                                        .border_1()
                                        .border_color(theme::border())
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::text())
                                        .child("Open")
                                        .on_click(move |_e, _w, cx| {
                                            if let Some(app) = app_open_with.upgrade() {
                                                app.update(cx, |this, _cx| {
                                                    this.open_selected_in_default_app();
                                                });
                                            }
                                        }),
                                ),
                        ),
                ),
        )
        // 2. Tab Navigation Segmented Bar (matching mockup: General, Camera, Location, Tags)
        .child(
            div()
                .w_full()
                .px_4()
                .py_2p5()
                .border_b_1()
                .border_color(theme::border_subtle())
                .child(render_tab_bar(weak_app.clone(), tab)),
        )
        // 3. Tab Content (Scrollable with smooth overflow)
        .child(
            div()
                .id("inspector-single-scroll-body")
                .flex_1()
                .w_full()
                .overflow_y_scroll()
                .p_4()
                .child(match tab {
                    InspectorTab::Info => render_info_tab(
                        weak_app.clone(),
                        &item,
                        &path,
                        &file_name,
                        &size_str,
                        is_trash,
                        rating,
                        &display_title,
                        caption,
                        caption_input_active,
                        caption_input_buffer,
                        title_input_active,
                        title_input_buffer,
                    ),
                    InspectorTab::Camera => render_camera_tab(&item, type_label),
                    InspectorTab::Location => render_location_tab(weak_app.clone(), &item),
                    InspectorTab::Tags => render_tags_tab(
                        weak_app.clone(),
                        tags,
                        library_tag_counts,
                        tag_input_active,
                        tag_input_buffer,
                    ),
                }),
        )
        .into_any_element()
}

fn render_tab_bar(weak_app: WeakEntity<LuminaApp>, active_tab: InspectorTab) -> impl IntoElement {
    let tabs = [
        (InspectorTab::Info, "General", "Info"),
        (InspectorTab::Camera, "Camera", "Camera"),
        (InspectorTab::Location, "Location", "Location"),
        (InspectorTab::Tags, "Tags", "Tags"),
    ];

    div()
        .w_full()
        .h(px(28.0))
        .bg(theme::bg_viewer())
        .border_1()
        .border_color(theme::border_subtle())
        .rounded_lg()
        .p_0p5()
        .flex()
        .flex_row()
        .gap_0p5()
        .children(tabs.into_iter().map(|(t, display_label, test_label)| {
            let is_active = t == active_tab;
            let app = weak_app.clone();
            div()
                .id(ElementId::Name(
                    format!("inspector-tab-{}", test_label).into(),
                ))
                .flex_1()
                .h_full()
                .flex()
                .justify_center()
                .items_center()
                .rounded_md()
                .cursor_pointer()
                .text_xs()
                .font_weight(if is_active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .bg(if is_active {
                    theme::bg_control()
                } else {
                    rgba(0x00000000)
                })
                .text_color(if is_active {
                    theme::text()
                } else {
                    theme::text_subtle()
                })
                .hover(|s| {
                    if is_active {
                        s
                    } else {
                        s.text_color(theme::text())
                    }
                })
                .child(display_label)
                .on_click(move |_e, _window, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| {
                            this.set_inspector_tab(t, cx);
                        });
                    }
                })
        }))
}
