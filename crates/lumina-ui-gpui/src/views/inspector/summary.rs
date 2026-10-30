//! Docked inspector body for a single item (v3): one column with the facts
//! people check while browsing. Full editing (caption, title, date, location)
//! stays in the tabbed Get Info window opened from the footer.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::app::types::format_duration_badge;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::body::{display_title, format_bytes, render_meta_row};
use crate::views::inspector::tabs::tags::render_tags_section;
use crate::views::inspector::tabs::trash::render_trash_lifecycle;
use crate::LuminaApp;

#[allow(clippy::too_many_arguments)]
pub fn render_summary_body(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    is_trash: bool,
    rating: u8,
    color_label: Option<u8>,
) -> AnyElement {
    let dims = if item.width > 0 && item.height > 0 {
        format!("{} × {}", item.width, item.height)
    } else {
        "—".to_string()
    };
    let place = match (&item.city, &item.country_code) {
        (Some(city), Some(cc)) => Some(format!("{city}, {cc}")),
        (Some(city), None) => Some(city.clone()),
        _ => None,
    };
    let app_info = weak_app.clone();
    let app_reveal = weak_app.clone();
    let path = item.path.clone();

    div()
        .id("inspector-summary")
        .debug_selector(|| "inspector-summary".to_string())
        .size_full()
        .overflow_y_scroll()
        .bg(theme::bg_pane())
        .text_color(theme::text())
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
                .child(render_stars(weak_app.clone(), rating))
                .child(div().flex_1())
                .child(render_label_dots(weak_app.clone(), color_label)),
        ))
        .child(section(
            "KEYWORDS",
            render_tags_section(
                weak_app.clone(),
                "ON THIS ITEM",
                tags,
                library_tag_counts,
                tag_input_active,
                tag_input_buffer,
            ),
        ))
        .children(place.map(|place| {
            section(
                "LOCATION",
                div().text_xs().text_color(theme::text_muted()).child(place),
            )
        }))
        .children(is_trash.then(|| render_trash_lifecycle(weak_app.clone())))
        // Footer: file facts and the way into the full editor.
        .child(
            div()
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
                                        app.update(cx, |this, _cx| {
                                            this.reveal_item_in_finder(&path)
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
        // ponytail: duration only; frame rate is not in the catalog yet.
        Some(format_duration_badge(item.duration_secs()))
            .filter(|d| !d.is_empty())
            .into_iter()
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
        .rounded(px(6.0))
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

fn render_stars(weak_app: WeakEntity<LuminaApp>, rating: u8) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .children((1u8..=5).map(move |star| {
            let app = weak_app.clone();
            let filled = rating >= star;
            let color = if filled {
                theme::rating()
            } else {
                theme::text_disabled()
            };
            div()
                .id(ElementId::NamedInteger(
                    "inspector-star".into(),
                    star as u64,
                ))
                .debug_selector(move || format!("inspector-star-{star}"))
                .cursor_pointer()
                .child(render_icon(
                    if filled { Icon::StarFilled } else { Icon::Star },
                    px(14.0),
                    color,
                ))
                .on_click(move |_e, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| this.set_rating_selected(star, cx));
                    }
                })
        }))
}

fn render_label_dots(weak_app: WeakEntity<LuminaApp>, color_label: Option<u8>) -> Div {
    div().flex().flex_row().items_center().gap_1p5().children(
        [
            (1u8, theme::label_red()),
            (2, theme::label_orange()),
            (3, theme::label_yellow()),
            (4, theme::label_green()),
            (5, theme::label_blue()),
            (6, theme::label_purple()),
        ]
        .into_iter()
        .map(move |(idx, color)| {
            let app = weak_app.clone();
            let on = color_label == Some(idx);
            div()
                .id(ElementId::NamedInteger(
                    "inspector-label".into(),
                    idx as u64,
                ))
                .debug_selector(move || format!("inspector-label-{idx}"))
                .size(px(14.0))
                .rounded_full()
                .bg(color)
                .cursor_pointer()
                .border_1()
                .border_color(if on {
                    theme::text()
                } else {
                    theme::border_subtle()
                })
                // Clicking the active label again clears it
                // (`set_color_label_selected` toggles).
                .on_click(move |_e, _w, cx| {
                    if let Some(app) = app.upgrade() {
                        app.update(cx, |this, cx| this.set_color_label_selected(idx, cx));
                    }
                })
        }),
    )
}
