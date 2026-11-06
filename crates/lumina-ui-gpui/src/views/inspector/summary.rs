//! Docked inspector body for a single item (v3): one column with the facts
//! people check while browsing. Full editing (caption, title, date, location)
//! stays in the tabbed Get Info window opened from the footer.

use crate::app::text_input::FieldEditor;
use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::types::format_duration_badge;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::body::{display_title, format_bytes, render_meta_row};
use crate::views::inspector::tabs::tags::{render_suggestions, Suggestion};
use crate::views::inspector::tabs::trash::render_trash_lifecycle;
use crate::LuminaApp;

#[allow(clippy::too_many_arguments)]
pub fn render_summary_body(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    tags: &[String],
    tag_input_active: bool,
    field_input: Option<FieldEditor>,
    (suggestions, suggestion_focus): (&[Suggestion], Option<usize>),
    is_trash: bool,
    rating: u8,
    star_pop: Option<(u8, ElementId)>,
    color_label: Option<u8>,
) -> AnyElement {
    let dims = if item.width > 0 && item.height > 0 {
        format!("{} × {}", item.width, item.height)
    } else {
        "—".to_string()
    };
    let place = place_line(item);
    let app_info = weak_app.clone();
    let app_reveal = weak_app.clone();
    let path = item.path.clone();

    let body = div()
        .id("inspector-summary-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p_4()
        .flex()
        .flex_col()
        .gap_4()
        // Title, filename, capture date.
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(display_title(item)),
                )
                .child(
                    div()
                        .id("inspector-filename")
                        .debug_selector(|| "inspector-filename".to_string())
                        .w_full()
                        // Lets a long name wrap instead of widening the panel.
                        .min_w(px(0.0))
                        .text_xs()
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::text_muted())
                        .child(item.file_name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child(item.formatted_timestamp()),
                ),
        )
        .child(render_stat_strip(item))
        .child(section(
            "RATING & LABEL",
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(render_stars(weak_app.clone(), rating, star_pop))
                .child(div().flex_1())
                .child(render_label_dots(weak_app.clone(), color_label)),
        ))
        .child(section(
            "TAGS",
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(render_tags(
                    weak_app.clone(),
                    tags,
                    tag_input_active,
                    field_input,
                ))
                // Only while typing: the docked summary stays compact.
                .children(
                    tag_input_active
                        .then(|| {
                            render_suggestions(weak_app.clone(), suggestions, suggestion_focus)
                        })
                        .flatten(),
                ),
        ))
        .children(place.map(|place| {
            section(
                "LOCATION",
                div().text_xs().text_color(theme::text_muted()).child(place),
            )
        }))
        .children(is_trash.then(|| render_trash_lifecycle(weak_app.clone())));

    div()
        .id("inspector-summary")
        .debug_selector(|| "inspector-summary".to_string())
        .size_full()
        .bg(theme::bg_pane())
        .text_color(theme::text())
        .flex()
        .flex_col()
        .child(body)
        // Footer pinned to the bottom: file facts and the way into Get Info.
        .child(
            div()
                .id("inspector-footer")
                .debug_selector(|| "inspector-footer".to_string())
                .flex_none()
                .p_4()
                .pt_2p5()
                .border_t_1()
                .border_color(theme::border_subtle())
                .flex()
                .flex_col()
                .gap_1p5()
                .child(render_meta_row("Size", &format_bytes(item.file_size)))
                .child(render_meta_row("Dimensions", &dims))
                .child(
                    div()
                        .pt_1p5()
                        .flex()
                        .flex_row()
                        .gap_1p5()
                        .child(
                            theme::control("inspector-get-info-btn")
                                .debug_selector(|| "inspector-get-info".to_string())
                                .flex_1()
                                .justify_center()
                                .tooltip(theme::tooltip_label("Open the full inspector window"))
                                .child("Get Info…")
                                .on_click(move |_e, _w, cx| {
                                    if let Some(app) = app_info.upgrade() {
                                        app.update(cx, |this, cx| this.undock_inspector(cx));
                                    }
                                }),
                        )
                        .child(
                            theme::control("inspector-reveal-btn")
                                .debug_selector(|| "inspector-reveal".to_string())
                                .flex_1()
                                .justify_center()
                                .child("Reveal")
                                .on_click(move |_e, _w, cx| {
                                    if let Some(app) = app_reveal.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.reveal_item_in_finder(&path, cx)
                                        });
                                    }
                                }),
                        ),
                ),
        )
        .into_any_element()
}

fn section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(theme::section_label(title))
        .child(content)
}

/// Mono cells with the numbers photographers scan for: exposure for photos,
/// duration for videos. Unknown values are left out, and the strip
/// is omitted entirely when nothing is known.
fn render_stat_strip(item: &MediaItem) -> AnyElement {
    let cells: Vec<String> = if item.media_type == MediaType::Video {
        // ponytail: no fps cell; frame rate is not in the catalog yet.
        [
            Some(format_duration_badge(item.duration_secs())).filter(|d| !d.is_empty()),
            (item.width > 0 && item.height > 0).then(|| resolution_label(item.width, item.height)),
        ]
        .into_iter()
        .flatten()
        .collect()
    } else {
        [
            item.f_number.map(|f| format!("ƒ/{f:.1}")),
            item.exposure_time.clone(),
            item.iso.map(|iso| format!("ISO {iso}")),
            item.focal_length.map(|fl| format!("{fl:.0} mm")),
        ]
        .into_iter()
        .flatten()
        .collect()
    };
    if cells.is_empty() {
        return Empty.into_any_element();
    }
    let last = cells.len() - 1;
    div()
        .debug_selector(|| "inspector-stats".to_string())
        .flex()
        .flex_row()
        .rounded(theme::RADIUS_MD)
        .border_1()
        .border_color(theme::border_subtle())
        .overflow_hidden()
        .font_family(theme::FONT_MONO)
        .text_xs()
        .children(cells.into_iter().enumerate().map(|(i, text)| {
            div()
                .flex_1()
                .py(px(7.0))
                .flex()
                .justify_center()
                .when(i < last, |d| {
                    d.border_r_1().border_color(theme::border_subtle())
                })
                .child(text)
        }))
        .into_any_element()
}

/// `pop` is the star just set and its pop id (`motion::MarkPop`).
pub(super) fn render_stars(
    weak_app: WeakEntity<LuminaApp>,
    rating: u8,
    pop: Option<(u8, ElementId)>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .children((1u8..=5).map(move |star| {
            let app = weak_app.clone();
            let filled = rating >= star;
            let color = if filled {
                theme::rating()
            } else {
                theme::rating_unset()
            };
            // 22px targets; clicking the current rating clears it.
            div()
                .id(ElementId::NamedInteger(
                    "inspector-star".into(),
                    star as u64,
                ))
                .debug_selector(move || format!("inspector-star-{star}"))
                .tooltip(theme::tooltip_label(if star == 1 {
                    "1 star".to_string()
                } else {
                    format!("{star} stars")
                }))
                .size(px(22.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(crate::motion::pop(
                    render_icon(
                        if filled { Icon::StarFilled } else { Icon::Star },
                        px(14.0),
                        color,
                    ),
                    pop.clone().filter(|(s, _)| *s == star).map(|(_, id)| id),
                ))
                .on_click(move |_e, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| this.set_rating_selected(star, cx));
                    }
                })
        }))
}

fn render_label_dots(weak_app: WeakEntity<LuminaApp>, color_label: Option<u8>) -> Div {
    let current = color_label.unwrap_or(0);
    div()
        .flex()
        .flex_row()
        .items_center()
        .children(theme::label_options().into_iter().map(move |label| {
            let app = weak_app.clone();
            let idx = label.0;
            // "No label" clears; the active colour toggles off
            // (`set_color_label_selected`).
            theme::label_swatch("inspector-label", label, current == idx).on_click(
                move |_e, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| this.set_color_label_selected(idx, cx));
                    }
                },
            )
        }))
}

/// "4K", "1080p" or "1920 × 1080" for the video strip.
pub(crate) fn resolution_label(width: u32, height: u32) -> String {
    match width.min(height) {
        2160 => "4K".to_string(),
        1440 | 1080 | 720 => format!("{}p", width.min(height)),
        _ => format!("{width} × {height}"),
    }
}

/// "Lisbon, Lisboa, PT": city, region (when it differs) and country code.
// ponytail: no neighbourhood or country names; the offline geocoder only
// has city, admin1 and ISO code.
pub(crate) fn place_line(item: &MediaItem) -> Option<String> {
    let city = item.city.as_ref()?;
    let mut parts = vec![city.clone()];
    if let Some(region) = item.admin1_region.as_ref().filter(|r| *r != city) {
        parts.push(region.clone());
    }
    parts.extend(item.country_code.clone());
    Some(parts.join(", "))
}

/// One compact field: plain chips (no "#") with ×, then an inline "add…".
fn render_tags(
    weak_app: WeakEntity<LuminaApp>,
    tags: &[String],
    input_active: bool,
    field_input: Option<FieldEditor>,
) -> impl IntoElement {
    let app_add = weak_app.clone();
    div()
        .id("inspector-tags")
        .debug_selector(|| "inspector-tags".to_string())
        .w_full()
        .min_h(px(30.0))
        .p(px(4.0))
        .rounded(theme::RADIUS_MD)
        .bg(theme::bg_app())
        .border_1()
        .border_color(if input_active {
            theme::focus_ring()
        } else {
            theme::border()
        })
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(4.0))
        .cursor_text()
        .children(tags.iter().map(|tag| {
            let app = weak_app.clone();
            let remove = tag.clone();
            let selector = format!("tag-chip-{tag}");
            div()
                .debug_selector(move || selector.clone())
                .h(px(20.0))
                .pl(px(7.0))
                .pr(px(3.0))
                .rounded(theme::RADIUS_SM)
                .bg(theme::bg_control_hover())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(3.0))
                .text_xs()
                .child({
                    let app = weak_app.clone();
                    let query = if tag.contains(' ') {
                        format!("tag:\"{tag}\"")
                    } else {
                        format!("tag:{tag}")
                    };
                    let selector = format!("tag-filter-{tag}");
                    div()
                        .id(ElementId::Name(selector.clone().into()))
                        .debug_selector(move || selector.clone())
                        .tooltip(theme::tooltip_label("Show items with this tag"))
                        .cursor_pointer()
                        .child(tag.clone())
                        .on_click(move |_e, _w, cx| {
                            cx.stop_propagation();
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |this, cx| this.set_search_query(&query, cx));
                            }
                        })
                })
                .child(
                    div()
                        .id(ElementId::Name(format!("tag-remove-{tag}").into()))
                        .debug_selector({
                            let id = format!("tag-remove-{tag}");
                            move || id.clone()
                        })
                        .tooltip(theme::tooltip_label("Remove tag"))
                        .size(px(14.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(render_icon(Icon::X, px(9.0), theme::text_muted()))
                        .on_click(move |_e, _w, cx| {
                            cx.stop_propagation();
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_remove_tag(remove.clone(), cx)
                                });
                            }
                        }),
                )
        }))
        .child(
            div()
                .id("tag-add")
                .debug_selector(|| "tag-add".to_string())
                .flex_1()
                .min_w(px(40.0))
                .px(px(4.0))
                .text_xs()
                .text_color(if input_active {
                    theme::text()
                } else {
                    theme::text_subtle()
                })
                .child(match field_input.filter(|_| input_active) {
                    Some(input) => input.into_any_element(),
                    None => "add…".into_any_element(),
                }),
        )
        // Clicking anywhere in the field starts typing (Enter adds, Esc stops).
        .on_click(move |_e, _w, cx| {
            if let Some(app) = app_add.upgrade() {
                app.update(cx, |this, cx| {
                    if !this.tag_input_active {
                        this.start_tag_input(cx);
                    }
                });
            }
        })
}
