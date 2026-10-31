//! v3 batch inspector: a flat panel for a multi-selection.

use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::body::format_bytes;
use crate::views::inspector::summary::render_stars;
use crate::LuminaApp;

const STACK_TILE: f32 = 44.0;
const STACK_STEP: f32 = 10.0;

/// Header, rating, tags, edit-all rows and a pinned selection hint. Takes no
/// `Context` so the docked panel and the inspector window share it.
pub fn render_batch(app: &LuminaApp, weak_app: WeakEntity<LuminaApp>) -> AnyElement {
    let summary = &app.batch_summary;
    let count = summary.count;
    let caption = &app.caption;
    div()
        .id("inspector-batch")
        .debug_selector(|| "inspector-batch".to_string())
        .size_full()
        .bg(theme::bg_pane())
        .text_color(theme::text())
        .flex()
        .flex_col()
        .child(
            div()
                .id("inspector-batch-scroll-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(16.0))
                .flex()
                .flex_col()
                .gap(px(18.0))
                .child(render_header(app))
                .child(section(
                    "RATING",
                    render_rating(app, weak_app.clone()),
                ))
                .child(section("TAGS", render_tags(app, weak_app.clone())))
                // Mixed-value-aware editing; applying a row writes one grouped
                // undo entry for the whole selection.
                .child(section(
                    "EDIT ALL",
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(render_batch_metadata_row(
                            BatchMetadataRow::Title,
                            "TITLE",
                            "batch-title-input",
                            &caption.title_input_buffer,
                            caption.title_input_active,
                            weak_app.clone(),
                        ))
                        .child(render_batch_metadata_row(
                            BatchMetadataRow::Caption,
                            "CAPTION",
                            "batch-caption-input",
                            &caption.input_buffer,
                            caption.input_active,
                            weak_app.clone(),
                        ))
                        .child(render_batch_metadata_row(
                            BatchMetadataRow::Date,
                            "DATE",
                            "batch-date-input",
                            &caption.date_input_buffer,
                            caption.date_input_active,
                            weak_app.clone(),
                        ))
                        .child(render_batch_metadata_row(
                            BatchMetadataRow::Location,
                            "LOCATION",
                            "batch-location-input",
                            &caption.location_input_buffer,
                            caption.location_input_active,
                            weak_app,
                        )),
                )),
        )
        .child(
            div()
                .id("inspector-batch-hint")
                .debug_selector(|| "inspector-batch-hint".to_string())
                .flex_none()
                .px(px(16.0))
                .py(px(10.0))
                .border_t_1()
                .border_color(theme::border_subtle())
                .text_xs()
                .text_color(theme::text_subtle())
                .child(format!(
                    "{count} selected · ⌘-click to add or remove · ⇧-click for a range · ⌘A selects all"
                )),
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

/// Three stacked thumbnails, "N items", the mix, and "size · date range".
fn render_header(app: &LuminaApp) -> Div {
    let summary = &app.batch_summary;
    let stack: Vec<_> = summary.ids.iter().take(3).copied().collect();
    let thumbs: Vec<Option<std::sync::Arc<RenderImage>>> = match &app.grid_state {
        Some(grid) => {
            let cache = grid.texture_cache.lock();
            stack.iter().map(|id| cache.peek(id).cloned()).collect()
        }
        None => vec![None; stack.len()],
    };
    for (id, thumb) in stack.iter().zip(&thumbs) {
        if thumb.is_none() {
            app.queue_thumbnail_request(*id);
        }
    }
    let photos = summary.photos_count + summary.raw_count;
    let videos = summary.videos_count;
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mix = match (photos, videos) {
        (_, 0) => plural(photos, "photo", "photos"),
        (0, _) => plural(videos, "video", "videos"),
        _ => format!(
            "{}, {}",
            plural(photos, "photo", "photos"),
            plural(videos, "video", "videos")
        ),
    };
    let width = STACK_TILE + STACK_STEP * thumbs.len().saturating_sub(1) as f32;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .id("batch-stack")
                .debug_selector(|| "batch-stack".to_string())
                .relative()
                .flex_none()
                .w(px(width))
                .h(px(STACK_TILE))
                .children(thumbs.into_iter().enumerate().map(|(i, thumb)| {
                    div()
                        .absolute()
                        .left(px(STACK_STEP * i as f32))
                        .top_0()
                        .size(px(STACK_TILE))
                        .rounded(px(6.0))
                        .overflow_hidden()
                        .border_2()
                        .border_color(theme::bg_pane())
                        .bg(theme::bg_raised())
                        .children(
                            thumb.map(|image| img(image).size_full().object_fit(ObjectFit::Cover)),
                        )
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .id("batch-count")
                        .debug_selector(|| "batch-count".to_string())
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(plural(summary.count, "item", "items")),
                )
                .child(
                    div()
                        .id("batch-mix")
                        .debug_selector(|| "batch-mix".to_string())
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(mix),
                )
                .child(
                    div()
                        .id("batch-size-range")
                        .debug_selector(|| "batch-size-range".to_string())
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child(format!(
                            "{} · {}",
                            format_bytes(summary.total_bytes),
                            date_range(summary.oldest_ms, summary.newest_ms)
                        )),
                ),
        )
}

/// The shared rating, or None when the selection disagrees.
pub(crate) fn common_rating(app: &LuminaApp) -> Option<u8> {
    // ponytail: one map lookup per selected id each frame; cache in the
    // summary if huge ⌘A selections make this show up in profiles.
    let mut ratings = app
        .batch_summary
        .ids
        .iter()
        .map(|id| app.item_ratings.get(id).copied().unwrap_or(0));
    let first = ratings.next()?;
    ratings.all(|r| r == first).then_some(first)
}

fn render_rating(app: &LuminaApp, weak_app: WeakEntity<LuminaApp>) -> Div {
    let common = common_rating(app);
    let note = match common {
        Some(0) => "unrated".to_string(),
        Some(1) => "1 star on all".to_string(),
        Some(n) => format!("{n} stars on all"),
        None => "mixed".to_string(),
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(render_stars(weak_app, common.unwrap_or(0)))
        .child(
            div()
                .id("batch-rating-note")
                .debug_selector(|| "batch-rating-note".to_string())
                .text_xs()
                .text_color(theme::text_muted())
                .child(note),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_disabled())
                .child("· click sets all"),
        )
}

/// One chip list sorted by count: solid chips are on every item, dashed
/// "n/N" chips on some (click applies to all); × removes from all.
fn render_tags(app: &LuminaApp, weak_app: WeakEntity<LuminaApp>) -> Div {
    let total = app.batch_summary.count;
    let mut counts = app.selected_tag_counts.clone();
    counts.sort_by(|(a, an), (b, bn)| bn.cmp(an).then_with(|| a.cmp(b)));
    let input_active = app.tag_input_active;
    let buffer = app.tag_input_buffer.clone();
    let app_add = weak_app.clone();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div().flex().flex_row().flex_wrap().gap_1p5().children(
                counts
                    .into_iter()
                    .map(|(tag, n)| tag_chip(weak_app.clone(), tag, n, total)),
            ),
        )
        .child(
            theme::field("batch-tag-add")
                .debug_selector(|| "batch-tag-add".to_string())
                .cursor_text()
                .border_color(if input_active {
                    theme::focus_ring()
                } else {
                    theme::border()
                })
                .text_color(if input_active && !buffer.is_empty() {
                    theme::text()
                } else {
                    theme::text_subtle()
                })
                .child(match (input_active, buffer.is_empty()) {
                    (true, true) => "|".to_string(),
                    (true, false) => format!("{buffer}|"),
                    (false, _) => format!("Add a tag to all {total}… (T)"),
                })
                .on_click(move |_e, _w, cx| {
                    if let Some(app) = app_add.upgrade() {
                        app.update(cx, |this, cx| {
                            if !this.tag_input_active {
                                this.start_tag_input(cx);
                            }
                        });
                    }
                }),
        )
}

fn tag_chip(weak_app: WeakEntity<LuminaApp>, tag: String, n: usize, total: usize) -> Stateful<Div> {
    let on_all = n >= total;
    // Dashed chips keep the `batch-partial-tag-*` automation names.
    let kind = if on_all { "tag" } else { "partial-tag" };
    let selector = format!("batch-{kind}-{tag}");
    let remove_selector = format!("batch-{kind}-remove-{tag}");
    let (add_app, add_tag) = (weak_app.clone(), tag.clone());
    let remove_tag = tag.clone();
    let chip = div()
        .id(ElementId::Name(selector.clone().into()))
        .debug_selector(move || selector.clone())
        .h(px(24.0))
        .px(px(9.0))
        .rounded_full()
        .border_1()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_xs();
    let chip = if on_all {
        chip.bg(theme::bg_control())
            .border_color(theme::border())
            .text_color(theme::text())
            .child(tag.clone())
    } else {
        chip.border_dashed()
            .border_color(theme::text_disabled())
            .text_color(theme::text_muted())
            .cursor_pointer()
            .hover(|s| s.bg(theme::bg_control_hover()))
            .tooltip(theme::tooltip_label(format!(
                "On {n} of {total} · add to all"
            )))
            .child(tag.clone())
            .child(
                div()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(11.0))
                    .text_color(theme::text_subtle())
                    .child(format!("{n}/{total}")),
            )
            .on_click(move |_e, _w, cx| {
                if let Some(app) = add_app.upgrade() {
                    app.update(cx, |this, cx| this.batch_add_tag(add_tag.clone(), cx));
                }
            })
    };
    chip.child(
        div()
            .id(ElementId::Name(remove_selector.clone().into()))
            .debug_selector(move || remove_selector.clone())
            .tooltip(theme::tooltip_label("Remove from all"))
            .cursor_pointer()
            .rounded_full()
            .hover(|s| s.bg(theme::border_strong()))
            .child(render_icon(Icon::X, px(10.0), theme::text_subtle()))
            .on_click(move |_e, _w, cx| {
                // A dashed chip would otherwise add the tag.
                cx.stop_propagation();
                if let Some(app) = weak_app.upgrade() {
                    app.update(cx, |this, cx| this.batch_remove_tag(remove_tag.clone(), cx));
                }
            }),
    )
}

/// "13 Jun 2022", "3 Mar – 13 Jun 2022" or "3 Mar 2021 – 13 Jun 2022" (UTC,
/// like the single-item capture time).
pub(crate) fn date_range(oldest_ms: i64, newest_ms: i64) -> String {
    use chrono::Datelike;
    let (Some(a), Some(b)) = (
        chrono::DateTime::from_timestamp_millis(oldest_ms),
        chrono::DateTime::from_timestamp_millis(newest_ms),
    ) else {
        return String::new();
    };
    if a.date_naive() == b.date_naive() {
        b.format("%-d %b %Y").to_string()
    } else if a.year() == b.year() {
        format!("{} – {}", a.format("%-d %b"), b.format("%-d %b %Y"))
    } else {
        format!("{} – {}", a.format("%-d %b %Y"), b.format("%-d %b %Y"))
    }
}

/// Which batch metadata field a row edits.
#[derive(Clone, Copy)]
enum BatchMetadataRow {
    Title,
    Caption,
    Date,
    Location,
}

/// One editable metadata row in the batch inspector.
///
/// The value comes from the shared inline buffers, kept in sync by the app: an
/// idle buffer holds the shared value or "Mixed", and the active buffer holds
/// the draft. Clicking opens the same inline editor the single-item inspector
/// uses, so the keyboard path (Enter apply, Escape cancel) is shared.
fn render_batch_metadata_row(
    row: BatchMetadataRow,
    label: &'static str,
    selector: &'static str,
    value: &str,
    editing: bool,
    weak_app: WeakEntity<LuminaApp>,
) -> AnyElement {
    let display = if value.is_empty() {
        "Not set".to_string()
    } else {
        value.to_string()
    };
    let mixed = !editing && value == "Mixed";
    let hint = if editing {
        "Enter to apply · Esc to cancel"
    } else {
        "Click to edit"
    };
    let app = weak_app;
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .w_full()
        .p_2()
        .rounded_md()
        .bg(if editing {
            theme::bg_control()
        } else {
            theme::bg_raised()
        })
        .border_1()
        .border_color(if editing {
            theme::focus_ring()
        } else {
            theme::border_subtle()
        })
        .cursor_pointer()
        .flex()
        .flex_col()
        .gap(px(3.0))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child(label),
                )
                .child(if mixed {
                    div()
                        .debug_selector(move || format!("{selector}-mixed"))
                        .px_1p5()
                        .py_0p5()
                        .rounded_full()
                        .bg(theme::accent())
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::accent_fg())
                        .child("Mixed")
                        .into_any_element()
                } else {
                    Empty.into_any_element()
                }),
        )
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if mixed && !editing {
                    theme::accent()
                } else {
                    theme::text()
                })
                .child(display),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_disabled())
                .child(hint),
        )
        .on_click(move |_e, _window, cx| {
            if let Some(app) = app.upgrade() {
                app.update(cx, |this, cx| match row {
                    BatchMetadataRow::Title => this.start_title_input(cx),
                    BatchMetadataRow::Caption => this.start_caption_input(cx),
                    BatchMetadataRow::Date => this.start_date_input(cx),
                    BatchMetadataRow::Location => this.start_location_input(cx),
                });
            }
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::date_range;

    #[core::prelude::v1::test]
    fn date_range_collapses_shared_day_and_year() {
        let day = 1_655_114_580_000; // 13 Jun 2022
        assert_eq!(date_range(day, day), "13 Jun 2022");
        let march = 1_646_300_000_000; // 3 Mar 2022
        assert_eq!(date_range(march, day), "3 Mar – 13 Jun 2022");
        let prior = 1_614_764_000_000; // 3 Mar 2021
        assert_eq!(date_range(prior, day), "3 Mar 2021 – 13 Jun 2022");
    }
}
