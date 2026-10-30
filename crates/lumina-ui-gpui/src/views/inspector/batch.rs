use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::body::{format_bytes, render_meta_row};
use crate::views::inspector::tabs::tags::render_tags_section;
use crate::{BatchSelectionSummary, LuminaApp};

#[allow(clippy::too_many_arguments)]
pub fn render_batch_body(
    weak_app: WeakEntity<LuminaApp>,
    summary: &BatchSelectionSummary,
    tags: &[String],
    tag_counts: &[(String, usize)],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    is_trash: bool,
    rating: u8,
    color_label: Option<u8>,
    caption_input_active: bool,
    caption_input_buffer: &str,
    title_input_active: bool,
    title_input_buffer: &str,
    date_input_active: bool,
    date_input_buffer: &str,
    location_input_active: bool,
    location_input_buffer: &str,
) -> AnyElement {
    let count = summary.count;
    let total_bytes = summary.total_bytes;
    let photos_count = summary.photos_count;
    let videos_count = summary.videos_count;
    let raw_count = summary.raw_count;
    let fav_count = summary.fav_count;
    // Without counts (not yet refreshed) every known tag is treated as on all.
    let (on_all, partial): (Vec<String>, Vec<(String, usize)>) = if tag_counts.is_empty() {
        (tags.to_vec(), Vec::new())
    } else {
        let (all, some): (Vec<_>, Vec<_>) =
            tag_counts.iter().cloned().partition(|(_, n)| *n >= count);
        (all.into_iter().map(|(tag, _)| tag).collect(), some)
    };

    let app_restore = weak_app.clone();
    let app_delete = weak_app.clone();
    let app_fav = weak_app.clone();
    let app_trash = weak_app.clone();

    div()
        .id("inspector-batch-scroll-body")
        .size_full()
        .overflow_y_scroll()
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        // Multi-Item Hero Card
        .child(
            div()
                .w_full()
                .p_3()
                .rounded_lg()
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border_subtle())
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child(format!("{} Media Items Selected", count)),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::text_muted())
                        .child(format!("Total Size: {}", format_bytes(total_bytes))),
                ),
        )
        // Batch Rating & Color Labels
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_2p5()
                .rounded_lg()
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border_subtle())
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
                                .child("BATCH RATING"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(2.0))
                                .children((1..=5).map(|star_val| {
                                    let app_star = weak_app.clone();
                                    let is_filled = star_val <= rating;
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "batch-star".into(),
                                            star_val as u64,
                                        ))
                                        .cursor_pointer()
                                        .px(px(2.0))
                                        .py(px(1.0))
                                        .text_sm()
                                        .text_color(if is_filled {
                                            theme::rating()
                                        } else {
                                            theme::text_disabled()
                                        })
                                        .hover(|s| s.text_color(theme::rating()))
                                        .child(render_icon(
                                            if is_filled {
                                                Icon::StarFilled
                                            } else {
                                                Icon::Star
                                            },
                                            px(14.0),
                                            if is_filled {
                                                theme::rating()
                                            } else {
                                                theme::text_disabled()
                                            },
                                        ))
                                        .on_click(move |_e, _window, cx| {
                                            if let Some(app) = app_star.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.set_rating_selected(star_val, cx);
                                                });
                                            }
                                        })
                                })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .pt_1()
                        .border_t_1()
                        .border_color(theme::border_subtle())
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text_subtle())
                                .child("COLOR LABEL"),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child({
                                    let app_clear = weak_app.clone();
                                    div()
                                        .id("batch-label-0")
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::text_disabled())
                                        .hover(|s| s.text_color(theme::text()))
                                        .child("⊘")
                                        .on_click(move |_e, _window, cx| {
                                            if let Some(app) = app_clear.upgrade() {
                                                app.update(cx, |this, cx| {
                                                    this.set_color_label_selected(0, cx);
                                                });
                                            }
                                        })
                                })
                                .children(
                                    [
                                        (1u8, theme::label_red()),
                                        (2, theme::label_orange()),
                                        (3, theme::label_yellow()),
                                        (4, theme::label_green()),
                                        (5, theme::label_blue()),
                                        (6, theme::label_purple()),
                                    ]
                                    .into_iter()
                                    .map(|(idx, color)| {
                                        let app_dot = weak_app.clone();
                                        let is_selected = color_label == Some(idx);
                                        div()
                                            .id(ElementId::NamedInteger(
                                                "batch-label-dot".into(),
                                                idx as u64,
                                            ))
                                            .size(px(12.0))
                                            .rounded_full()
                                            .bg(color)
                                            .cursor_pointer()
                                            .border_1()
                                            .border_color(if is_selected {
                                                theme::text()
                                            } else {
                                                theme::border_subtle()
                                            })
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .child(if is_selected {
                                                div()
                                                    .size(px(4.0))
                                                    .rounded_full()
                                                    .bg(theme::text())
                                                    .into_any_element()
                                            } else {
                                                Empty.into_any_element()
                                            })
                                            .on_click(move |_e, _window, cx| {
                                                if let Some(app) = app_dot.upgrade() {
                                                    app.update(cx, |this, cx| {
                                                        this.set_color_label_selected(idx, cx);
                                                    });
                                                }
                                            })
                                    }),
                                ),
                        ),
                ),
        )
        // Keywords: tags on every selected item use the normal editor (× removes
        // from all); tags on only some are listed below it as dashed chips.
        .child(render_tags_section(
            weak_app.clone(),
            &on_all,
            library_tag_counts,
            tag_input_active,
            tag_input_buffer,
        ))
        .children(
            (!partial.is_empty()).then(|| render_partial_tags(weak_app.clone(), &partial, count)),
        )
        // Mixed-value-aware metadata editing. Applying either field writes one
        // grouped undo entry for the whole selection.
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_2p5()
                .rounded_lg()
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border_subtle())
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
                                .child("BATCH METADATA"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme::text_disabled())
                                .child(format!("Applies to all {} · one undo step", count)),
                        ),
                )
                .child(render_batch_metadata_row(
                    BatchMetadataRow::Title,
                    "TITLE",
                    "batch-title-input",
                    title_input_buffer,
                    title_input_active,
                    weak_app.clone(),
                ))
                .child(render_batch_metadata_row(
                    BatchMetadataRow::Caption,
                    "CAPTION",
                    "batch-caption-input",
                    caption_input_buffer,
                    caption_input_active,
                    weak_app.clone(),
                ))
                .child(render_batch_metadata_row(
                    BatchMetadataRow::Date,
                    "DATE",
                    "batch-date-input",
                    date_input_buffer,
                    date_input_active,
                    weak_app.clone(),
                ))
                .child(render_batch_metadata_row(
                    BatchMetadataRow::Location,
                    "LOCATION",
                    "batch-location-input",
                    location_input_buffer,
                    location_input_active,
                    weak_app.clone(),
                )),
        )
        // Batch Actions Box
        .child(if is_trash {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .id("batch-restore-inspect-btn")
                        .flex_1()
                        .py_2()
                        .bg(theme::selection_bg())
                        .hover(|s| s.bg(theme::selection_bg_hover()))
                        .rounded_md()
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::accent())
                        .child(render_icon(Icon::Undo2, px(12.0), theme::accent()))
                        .child("Restore")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_restore.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.restore_selected_trash(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("batch-delete-inspect-btn")
                        .flex_1()
                        .py_2()
                        .bg(theme::danger_bg())
                        .hover(|s| s.bg(theme::danger_bg_hover()))
                        .rounded_md()
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child(render_icon(Icon::X, px(12.0), theme::text()))
                        .child("Delete Now")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_delete.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.prompt_delete_selected(cx);
                                });
                            }
                        }),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .id("batch-fav-inspect-btn")
                        .flex_1()
                        .py_2()
                        .bg(theme::bg_control())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .rounded_md()
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme::rating())
                        .child(render_icon(Icon::HeartFilled, px(12.0), theme::rating()))
                        .child(format!("Fav ({})", fav_count))
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_fav.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_toggle_favorite(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("batch-trash-inspect-btn")
                        .flex_1()
                        .py_2()
                        .bg(theme::bg_control())
                        .hover(|s| s.bg(theme::danger_bg()))
                        .rounded_md()
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .text_color(theme::danger_text())
                        .child(render_icon(Icon::Trash, px(12.0), theme::danger_text()))
                        .child("Trash")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_trash.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_trash_or_restore(cx);
                                });
                            }
                        }),
                )
                .into_any_element()
        })
        // Aggregated Composition Section
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1p5()
                .p_2p5()
                .rounded_lg()
                .bg(theme::bg_pane())
                .border_1()
                .border_color(theme::border_subtle())
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SELECTION BREAKDOWN"),
                )
                .child(render_meta_row("Photos", &photos_count.to_string()))
                .child(render_meta_row("Videos", &videos_count.to_string()))
                .child(render_meta_row("RAW Media", &raw_count.to_string()))
                .child(render_meta_row(
                    "Favorited",
                    &format!("{}/{}", fav_count, count),
                )),
        )
        .into_any_element()
}

/// Tags on only some of the selection: dashed chips with an `n/total` count.
/// Clicking one adds it to every selected item.
fn render_partial_tags(
    weak_app: WeakEntity<LuminaApp>,
    partial: &[(String, usize)],
    total: usize,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("On some items · click to apply to all"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(partial.iter().map(|(tag, n)| {
                    let app = weak_app.clone();
                    let tag_for_click = tag.clone();
                    let selector = format!("batch-partial-tag-{tag}");
                    div()
                        .id(ElementId::Name(selector.clone().into()))
                        .debug_selector(move || selector.clone())
                        .tooltip(theme::tooltip_label(format!(
                            "On {n} of {total} · add to all"
                        )))
                        .h(px(24.0))
                        .px(px(9.0))
                        .rounded_full()
                        .border_1()
                        .border_dashed()
                        .border_color(theme::text_disabled())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(tag.clone())
                        .child(
                            div()
                                .font_family(theme::FONT_MONO)
                                .text_size(px(10.0))
                                .text_color(theme::text_subtle())
                                .child(format!("{n}/{total}")),
                        )
                        .on_click(move |_e, _w, cx| {
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_add_tag(tag_for_click.clone(), cx)
                                });
                            }
                        })
                })),
        )
        .into_any_element()
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
            theme::accent()
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
                .text_color(if editing || mixed {
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
