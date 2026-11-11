//! Docked inspector body for a single item (Detail design): file header,
//! Capture, Rating & Label, Location and Tags. Full editing (caption, title,
//! date, location) stays in the tabbed Get Info window opened from the footer.

use crate::app::text_input::FieldEditor;
use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme::{self, ChipKind};
use crate::views::inspector::body::{format_bytes, render_meta_row};
use crate::views::inspector::capture::capture_rows;
use crate::views::inspector::tabs::tags::{render_suggestions, Suggestion};
use crate::views::inspector::tabs::trash::render_trash_lifecycle;
use crate::LuminaApp;

/// `focal_35mm` comes from `InfoDetails`, read off the UI thread; `None`
/// until it lands or when the file does not record it.
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
    focal_35mm: Option<u32>,
) -> AnyElement {
    let place = place_line(item);
    let capture = capture_rows(item, focal_35mm);
    let app_info = weak_app.clone();
    let app_reveal = weak_app.clone();
    let path = item.path.clone();

    let body = div()
        .id("inspector-summary-scroll")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .p(px(20.0))
        .flex()
        .flex_col()
        .gap(px(22.0))
        .child(render_file_header(item))
        // Rows are omitted one by one and the section with them: no "—" rows.
        .when(!capture.is_empty(), |body| {
            body.child(
                section(
                    "inspector.capture",
                    div()
                        .flex()
                        .flex_col()
                        .children(capture.into_iter().map(|row| {
                            let selector = row.selector;
                            render_meta_row(&row.key, &row.value)
                                .debug_selector(move || selector.to_string())
                        })),
                )
                .debug_selector(|| "inspector-capture".to_string()),
            )
        })
        .child(section(
            "inspector.rating",
            div()
                .flex()
                .flex_row()
                .items_center()
                .child(render_stars(weak_app.clone(), rating, star_pop))
                .child(div().flex_1())
                .child(render_label_dots(weak_app.clone(), color_label)),
        ))
        .children(place.map(|place| {
            section(
                "inspector.location",
                div().text_xs().text_color(theme::t2()).child(place),
            )
        }))
        .child(section(
            "inspector.tags",
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
        .children(is_trash.then(|| render_trash_lifecycle(weak_app.clone())));

    div()
        .id("inspector-summary")
        .debug_selector(|| "inspector-summary".to_string())
        .size_full()
        .bg(theme::bg_1())
        .text_color(theme::t1())
        .flex()
        .flex_col()
        .child(body)
        // Slim footer pinned to the bottom: Get Info is the only mouse path
        // to editing caption, date and location.
        .child(
            div()
                .id("inspector-footer")
                .debug_selector(|| "inspector-footer".to_string())
                .flex_none()
                .px(px(20.0))
                .py(px(12.0))
                .border_t_1()
                .border_color(theme::line())
                .flex()
                .flex_row()
                .gap_1p5()
                .child(
                    theme::control("inspector-get-info-btn")
                        .debug_selector(|| "inspector-get-info".to_string())
                        .flex_1()
                        .justify_center()
                        .tooltip(theme::tooltip_label(t("inspector.get_info_tooltip")))
                        .child(t("inspector.get_info"))
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
                        .child(t("inspector.reveal"))
                        .on_click(move |_e, _w, cx| {
                            if let Some(app) = app_reveal.upgrade() {
                                app.update(cx, |this, cx| this.reveal_item_in_finder(&path, cx));
                            }
                        }),
                ),
        )
        .into_any_element()
}

/// Filename with its format badge, then the When line. A long name wraps
/// beside the badge, so the badge stays visible.
fn render_file_header(item: &MediaItem) -> Div {
    let dims =
        (item.width > 0 && item.height > 0).then(|| format!("{} × {}", item.width, item.height));
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.0))
                .child(
                    div()
                        .id("inspector-filename")
                        .debug_selector(|| "inspector-filename".to_string())
                        .flex_1()
                        // Lets a long name wrap instead of widening the panel.
                        .min_w(px(0.0))
                        .text_size(px(13.0))
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::t1())
                        .child(item.file_name.clone()),
                )
                .child(
                    div()
                        .id("inspector-format-badge")
                        .debug_selector(|| "inspector-format-badge".to_string())
                        .flex_none()
                        .h(px(18.0))
                        .px(px(6.0))
                        .rounded(px(4.0))
                        .bg(theme::accent_soft())
                        .flex()
                        .items_center()
                        .text_size(px(9.5))
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::accent())
                        .child(format_badge(item))
                        // Dimensions moved here from the old footer.
                        .when_some(dims, |badge, dims| {
                            badge.tooltip(theme::tooltip_label(dims))
                        }),
                ),
        )
        .child(
            div()
                .debug_selector(|| "inspector-when".to_string())
                .text_size(px(10.5))
                .font_family(theme::FONT_MONO)
                .text_color(theme::t3())
                .child(when_line(item)),
        )
}

/// "RAW" for camera raw files, else the uppercased extension ("JPG", "MOV").
pub(crate) fn format_badge(item: &MediaItem) -> String {
    if item.media_type == MediaType::Raw {
        return "RAW".to_string();
    }
    std::path::Path::new(&item.file_name)
        .extension()
        .map(|ext| ext.to_string_lossy().to_uppercase())
        .unwrap_or_default()
}

/// "Sat 14 Jun 2025 · 18:42:09 · 48.2 MB" (UTC, like the rest of the app).
pub(crate) fn when_line(item: &MediaItem) -> String {
    let when = chrono::DateTime::from_timestamp_millis(item.timestamp_epoch_ms)
        .map(|dt| dt.format("%a %-d %b %Y · %H:%M:%S").to_string())
        .unwrap_or_else(|| format!("{:04}-{:02}-{:02}", item.year, item.month, item.day));
    format!("{when} · {}", format_bytes(item.file_size))
}

/// Section with the design title: 9.5/600 `t3`, uppercase in the catalog.
// ponytail: no letter spacing (design: 1); GPUI text has no tracking.
fn section(title_key: &str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(9.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t3())
                .child(t(title_key)),
        )
        .child(content)
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

/// Design chips ("#" glyph, label, ×) that wrap onto new rows, then an "Add"
/// outline chip that turns into the input. Clicking anywhere in the field
/// starts typing; Enter adds (or takes the highlighted suggestion), Esc stops,
/// ⌫ in the empty field removes the last tag (app/keymap/modals.rs).
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
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(6.0))
        .cursor_text()
        .children(tags.iter().map(|tag| {
            let selector = format!("tag-chip-{tag}");
            theme::chip(ElementId::Name(selector.clone().into()), ChipKind::Normal)
                .debug_selector(move || selector.clone())
                // A long tag truncates inside one panel-wide chip.
                .max_w_full()
                .child(
                    div()
                        .flex_none()
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::accent())
                        .child("#"),
                )
                .child(render_tag_filter(weak_app.clone(), tag))
                .child(render_tag_remove(weak_app.clone(), tag.clone()))
        }))
        .child({
            let add = theme::chip("tag-add", ChipKind::Ghost)
                .debug_selector(|| "tag-add".to_string())
                .border_color(theme::line_2())
                .text_color(theme::t3());
            match field_input.filter(|_| input_active) {
                Some(input) => add
                    .flex_1()
                    .min_w(px(120.0))
                    .border_color(theme::focus_ring())
                    .text_color(theme::t1())
                    .child(div().flex_1().min_w_0().child(input)),
                None => add
                    .child(render_icon(Icon::Plus, px(11.0), theme::t3()))
                    .child(t("inspector.tag_add")),
            }
        })
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

/// The chip's label: clicking it searches for the tag.
fn render_tag_filter(weak_app: WeakEntity<LuminaApp>, tag: &str) -> Stateful<Div> {
    let query = if tag.contains(' ') {
        format!("tag:\"{tag}\"")
    } else {
        format!("tag:{tag}")
    };
    let selector = format!("tag-filter-{tag}");
    div()
        .id(ElementId::Name(selector.clone().into()))
        .debug_selector(move || selector.clone())
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .tooltip(theme::tooltip_label(t("inspector.filter_tag")))
        .cursor_pointer()
        .child(tag.to_string())
        .on_click(move |_e, _w, cx| {
            cx.stop_propagation();
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| this.set_search_query(&query, cx));
            }
        })
}

fn render_tag_remove(weak_app: WeakEntity<LuminaApp>, tag: String) -> Stateful<Div> {
    let selector = format!("tag-remove-{tag}");
    div()
        .id(ElementId::Name(selector.clone().into()))
        .debug_selector(move || selector.clone())
        .tooltip(theme::tooltip_label(t("inspector.remove_tag")))
        .flex_none()
        .size(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover(|s| s.bg(theme::line_2()))
        .child(render_icon(Icon::X, px(9.0), theme::t2()))
        .on_click(move |_e, _w, cx| {
            cx.stop_propagation();
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| this.batch_remove_tag(tag.clone(), cx));
            }
        })
}
