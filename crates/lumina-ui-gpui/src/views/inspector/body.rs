use std::sync::Arc;

use crate::app::text_input::FieldEditor;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::info_details::InfoDetails;
pub use crate::app::types::format_bytes;
use crate::icons::{render_icon, Icon};
use crate::motion::MarkPop;
use crate::theme;
use crate::views::inspector::tabs::camera::render_camera_tab;
use crate::views::inspector::tabs::info::render_info_tab;
use crate::views::inspector::tabs::location::render_location_tab;
use crate::views::inspector::tabs::tags::{render_tags_tab, Suggestion};
use crate::LuminaApp;

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

/// Inspector heading: the user title, then the place, then the filename stem.
pub(crate) fn display_title(item: &MediaItem) -> String {
    if let Some(ref title) = item.title {
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
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_inspector_body(
    weak_app: WeakEntity<LuminaApp>,
    selected_item: Option<MediaItem>,
    preview_image: Option<Arc<RenderImage>>,
    tags: &[String],
    suggestions: &[Suggestion],
    suggestion_focus: Option<usize>,
    tag_input_active: bool,
    is_trash: bool,
    tab: InspectorTab,
    rating: u8,
    mark_pop: &MarkPop,
    caption: Option<&str>,
    caption_input_active: bool,
    title_input_active: bool,
    date_input_active: bool,
    location_input_active: bool,
    field_input: Option<FieldEditor>,
    details: Option<&InfoDetails>,
) -> AnyElement {
    let item = match selected_item {
        Some(it) => it,
        None => {
            return div()
                .size_full()
                .flex()
                .justify_center()
                .items_center()
                .text_sm()
                .text_color(theme::t2())
                .child("No media selected")
                .into_any_element();
        }
    };

    let details = details.filter(|d| d.is_for(&item));
    render_single_body(
        weak_app,
        item,
        preview_image,
        tags,
        suggestions,
        suggestion_focus,
        tag_input_active,
        is_trash,
        tab,
        rating,
        mark_pop,
        caption,
        caption_input_active,
        title_input_active,
        date_input_active,
        location_input_active,
        field_input,
        details,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_single_body(
    weak_app: WeakEntity<LuminaApp>,
    item: MediaItem,
    preview_image: Option<Arc<RenderImage>>,
    tags: &[String],
    suggestions: &[Suggestion],
    suggestion_focus: Option<usize>,
    tag_input_active: bool,
    is_trash: bool,
    tab: InspectorTab,
    rating: u8,
    mark_pop: &MarkPop,
    caption: Option<&str>,
    caption_input_active: bool,
    title_input_active: bool,
    date_input_active: bool,
    location_input_active: bool,
    field_input: Option<FieldEditor>,
    details: Option<&InfoDetails>,
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

    let display_title = display_title(&item);

    let dim_str = if item.width > 0 && item.height > 0 {
        format!("{} × {}", item.width, item.height)
    } else {
        "—".to_string()
    };
    // Thumbnail keeps the media's aspect inside a 132×99 box.
    let (thumb_w, thumb_h) = if item.width > 0 && item.height > 0 {
        let aspect = item.width as f32 / item.height as f32;
        if aspect >= 132.0 / 99.0 {
            (132.0, 132.0 / aspect)
        } else {
            (99.0 * aspect, 99.0)
        }
    } else {
        (132.0, 99.0)
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
        .bg(theme::bg_1())
        .text_color(theme::t1())
        .overflow_hidden()
        // 1. Header: aspect-shaped thumbnail, titles, quick actions (v3).
        .child(
            div()
                .w_full()
                .p_4()
                .border_b_1()
                .border_color(theme::line())
                .flex()
                .flex_row()
                .gap_4()
                .items_start()
                .child(
                    div()
                        .w(px(132.0))
                        .h(px(99.0))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .id("info-thumbnail")
                                .debug_selector(|| "info-thumbnail".to_string())
                                .w(px(thumb_w))
                                .h(px(thumb_h))
                                .overflow_hidden()
                                .border_1()
                                .border_color(theme::line())
                                .bg(theme::bg_0())
                                .children(preview_image.clone().map(|img_render| {
                                    img(img_render).size_full().object_fit(ObjectFit::Cover)
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        // Without min_w(0) the automatic flex minimum is the
                        // text's unwrapped min-content width, so a long name
                        // widens the row instead of wrapping (BUG 12).
                        .min_w(px(0.0))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .w_full()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::t1())
                                .child(display_title.clone()),
                        )
                        .child(
                            div()
                                .id("inspector-filename")
                                .debug_selector(|| "inspector-filename".to_string())
                                .w_full()
                                .text_xs()
                                .font_family(theme::FONT_MONO)
                                .text_color(theme::t2())
                                .child(file_name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::t2())
                                .child(meta_summary_line),
                        )
                        .child(
                            div()
                                .pt_1p5()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .flex_wrap()
                                .child(
                                    header_button(
                                        "info-quick-fav-btn",
                                        if is_fav { "Favorited" } else { "Favorite" },
                                        Some(if is_fav {
                                            Icon::HeartFilled
                                        } else {
                                            Icon::Heart
                                        }),
                                        is_fav,
                                        mark_pop.heart(Some(item.id)),
                                    )
                                    .tooltip(theme::tooltip_label("Favorite (F)"))
                                    .on_click(
                                        move |_e, _w, cx| {
                                            if let Some(app) = app_fav.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.toggle_favorite_selected(cx);
                                                });
                                            }
                                        },
                                    ),
                                )
                                .child(
                                    header_button(
                                        "info-quick-reveal-btn",
                                        crate::i18n::platform::REVEAL,
                                        None,
                                        false,
                                        None,
                                    )
                                    .tooltip(theme::tooltip_label(
                                        crate::i18n::platform::REVEAL_TOOLTIP,
                                    ))
                                    .on_click(
                                        move |_e, _w, cx| {
                                            if let Some(app) = app_reveal.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.reveal_item_in_finder(
                                                        &path_for_reveal,
                                                        cx,
                                                    );
                                                });
                                            }
                                        },
                                    ),
                                )
                                .child(
                                    header_button(
                                        "info-quick-copy-path-btn",
                                        "Copy Path",
                                        None,
                                        false,
                                        None,
                                    )
                                    .tooltip(theme::tooltip_label("Copy Path (⌥⌘C)"))
                                    .on_click(
                                        move |_e, _w, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                path_for_copy.display().to_string(),
                                            ));
                                        },
                                    ),
                                )
                                // ponytail: "Open With ▾" needs a native app-list
                                // bridge; this opens the default app.
                                .child(
                                    header_button(
                                        "info-quick-open-with-btn",
                                        "Open",
                                        None,
                                        false,
                                        None,
                                    )
                                    .tooltip(theme::tooltip_label("Open in Default App"))
                                    .on_click(
                                        move |_e, _w, cx| {
                                            if let Some(app) = app_open_with.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.open_selected_in_default_app(cx);
                                                });
                                            }
                                        },
                                    ),
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
                .border_color(theme::line())
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
                        mark_pop.star(Some(item.id)),
                        &display_title,
                        caption,
                        caption_input_active,
                        title_input_active,
                        date_input_active,
                        field_input.clone(),
                        details,
                    ),
                    InspectorTab::Camera => render_camera_tab(&item, details),
                    InspectorTab::Location => render_location_tab(
                        weak_app.clone(),
                        &item,
                        location_input_active,
                        field_input.clone(),
                    ),
                    InspectorTab::Tags => render_tags_tab(
                        weak_app.clone(),
                        tags,
                        suggestions,
                        suggestion_focus,
                        tag_input_active,
                        field_input,
                    ),
                }),
        )
        .into_any_element()
}

/// Header action: 6px radius (not a pill); `on` is the amber "Favorited" fill.
/// `pop` replays the icon's pop (`motion::MarkPop`).
fn header_button(
    id: &'static str,
    label: &str,
    icon: Option<Icon>,
    on: bool,
    pop: Option<ElementId>,
) -> Stateful<Div> {
    let fg = if on { theme::on_accent() } else { theme::t1() };
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .h(px(26.0))
        .px_2p5()
        .rounded(theme::RADIUS_MD)
        .border_1()
        .border_color(if on { theme::accent() } else { theme::line() })
        .bg(if on { theme::accent() } else { theme::bg_2() })
        .hover(move |s| if on { s } else { s.bg(theme::bg_3()) })
        .cursor_pointer()
        .text_xs()
        .text_color(fg)
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .children(icon.map(|icon| crate::motion::pop(render_icon(icon, px(12.0), fg), pop)))
        .child(label.to_string())
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
        .bg(theme::bg_stage())
        .border_1()
        .border_color(theme::line())
        .rounded(theme::RADIUS_MD)
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
                .debug_selector(move || format!("inspector-tab-{}", test_label))
                .flex_1()
                .h_full()
                .flex()
                .justify_center()
                .items_center()
                .rounded(theme::RADIUS_MD)
                .cursor_pointer()
                .text_xs()
                .font_weight(if is_active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .bg(if is_active {
                    theme::bg_2()
                } else {
                    rgba(0x00000000)
                })
                .text_color(if is_active { theme::t1() } else { theme::t2() })
                .hover(|s| {
                    if is_active {
                        s
                    } else {
                        s.text_color(theme::t1())
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
