use gpui::*;
use lumina_core::{MediaId, MediaItem};

use crate::app::types::format_bytes;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub struct DuplicatesView;

impl DuplicatesView {
    pub fn render(
        this: &LuminaApp,
        total_count: usize,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        if this.duplicate_groups.is_empty() && total_count == 0 {
            return this.render_empty_state(cx);
        }

        let total_groups = this.duplicate_groups.len();
        let total_items = this.duplicate_items_count;
        let total_savings: u64 = this
            .duplicate_groups
            .iter()
            .map(|g| g.potential_reclaimable_bytes)
            .sum();

        div()
            .id("duplicates-view")
            .debug_selector(|| "duplicates-view".to_string())
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .bg(theme::bg_app())
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            // Banner Header
            .child(
                div()
                    .debug_selector(|| "duplicates-banner".to_string())
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .p_3()
                    .rounded(theme::RADIUS_LG)
                    .bg(theme::bg_raised())
                    .border_1()
                    .border_color(theme::border_subtle())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(render_icon(Icon::Repeat, px(18.0), theme::accent()))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme::text())
                                            .child(format!(
                                                "{total_groups} Duplicate Set{} ({total_items} Items)",
                                                if total_groups == 1 { "" } else { "s" }
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text_subtle())
                                            .child(format!(
                                                "Merging will reclaim {} by moving redundant copies to Recently Deleted.",
                                                format_bytes(total_savings)
                                            )),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("merge-all-duplicates-btn")
                            .debug_selector(|| "merge-all-duplicates".to_string())
                            .px_3()
                            .py_1p5()
                            .rounded(theme::RADIUS_MD)
                            .bg(theme::accent())
                            .hover(|s| s.bg(theme::accent_hover()))
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::accent_fg())
                            .child("Merge All Duplicates")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.merge_all_duplicates(cx);
                            })),
                    ),
            )
            // Duplicate Groups List
            .children(this.duplicate_groups.iter().enumerate().map(|(idx, group)| {
                Self::render_group(this, idx, group, cx)
            }))
            .into_any_element()
    }

    fn render_group(
        this: &LuminaApp,
        group_idx: usize,
        group: &lumina_core::DuplicateGroup,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let keeper_id = group.items.first().map(|i| i.id).unwrap_or(MediaId(0));
        let trash_ids: Vec<MediaId> = group.items.iter().skip(1).map(|i| i.id).collect();
        let item_count = group.items.len();
        let group_size = group.file_size;
        let savings = group.potential_reclaimable_bytes;

        div()
            .id(ElementId::Name(format!("dup-group-{}", group_idx).into()))
            .debug_selector(move || format!("duplicate-group-{}", group_idx))
            .rounded(theme::RADIUS_LG)
            .bg(theme::bg_pane())
            .border_1()
            .border_color(theme::border_subtle())
            .p_3()
            .flex()
            .flex_col()
            .gap_3()
            // Group Header
            .child(
                div()
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
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::text())
                                    .child(format!("Exact Match · {item_count} copies")),
                            )
                            .child(div().text_xs().text_color(theme::text_subtle()).child(
                                format!(
                                    "{} each · {} reclaimable",
                                    format_bytes(group_size),
                                    format_bytes(savings)
                                ),
                            )),
                    )
                    .child(
                        div()
                            .id(ElementId::Name(
                                format!("merge-group-btn-{}", group_idx).into(),
                            ))
                            .debug_selector(move || format!("merge-group-{}", group_idx))
                            .px_2p5()
                            .py_1()
                            .rounded(theme::RADIUS_SM)
                            .bg(theme::bg_raised())
                            .hover(|s| s.bg(theme::accent()).text_color(theme::accent_fg()))
                            .border_1()
                            .border_color(theme::border())
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            .child("Merge")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.merge_duplicate_group(keeper_id, &trash_ids, cx);
                            })),
                    ),
            )
            // Group Cards
            .child(
                div().flex().flex_row().flex_wrap().gap_3().children(
                    group.items.iter().enumerate().map(|(item_idx, item)| {
                        Self::render_item_card(this, item, item_idx == 0, cx)
                    }),
                ),
            )
    }

    fn render_item_card(
        this: &LuminaApp,
        item: &MediaItem,
        is_keeper: bool,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let item_id = item.id;
        let item_id_num = item_id.0;
        let item_clone = item.clone();
        let is_selected = this
            .selected_item
            .as_ref()
            .map(|i| i.id == item_id)
            .unwrap_or(false);

        // Thumbnail request
        let texture = if let Some(ref state) = this.grid_state {
            let mut cache = state.texture_cache.lock();
            let tex = cache.get(&item_id).cloned();
            drop(cache);
            if tex.is_none() {
                this.queue_thumbnail_request_item(item);
            }
            tex
        } else {
            None
        };

        let formatted_date = item.formatted_timestamp();
        let dimensions = if item.width > 0 && item.height > 0 {
            format!("{} × {}", item.width, item.height)
        } else {
            String::new()
        };

        div()
            .id(ElementId::Name(format!("dup-card-{}", item_id_num).into()))
            .debug_selector(move || format!("duplicate-card-{}", item_id_num))
            .w(px(200.0))
            .rounded(theme::RADIUS_MD)
            .bg(if is_selected {
                theme::selection_bg()
            } else {
                theme::bg_raised()
            })
            .border_1()
            .border_color(if is_selected {
                theme::focus_ring()
            } else {
                theme::border()
            })
            .overflow_hidden()
            .flex()
            .flex_col()
            .cursor_pointer()
            .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
                if e.click_count() >= 2 {
                    this.open_viewer(item_clone.clone(), cx);
                } else {
                    this.selected_item = Some(item_clone.clone());
                    this.selected_index = None;
                    this.selected_indices.clear();
                    this.refresh_selected_tags();
                    cx.notify();
                }
            }))
            // Thumbnail container
            .child(
                div()
                    .w_full()
                    .h(px(130.0))
                    .bg(theme::bg_app())
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(if let Some(tex) = texture {
                        img(tex).size_full().into_any_element()
                    } else {
                        div()
                            .size_full()
                            .bg(theme::bg_raised())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render_icon(Icon::Image, px(24.0), theme::text_subtle()))
                            .into_any_element()
                    })
                    // Keeper / Copy badge
                    .child(
                        div()
                            .absolute()
                            .top_2()
                            .left_2()
                            .px_1p5()
                            .py_0p5()
                            .rounded(theme::RADIUS_SM)
                            .bg(if is_keeper {
                                theme::accent()
                            } else {
                                theme::bg_raised()
                            })
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if is_keeper {
                                theme::accent_fg()
                            } else {
                                theme::text_muted()
                            })
                            .child(if is_keeper { "Primary" } else { "Copy" }),
                    ),
            )
            // Metadata Details
            .child(
                div()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text())
                            .truncate()
                            .child(item.file_name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_subtle())
                            .truncate()
                            .child(format!("{dimensions} · {}", format_bytes(item.file_size))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::text_subtle())
                            .truncate()
                            .child(formatted_date),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family(theme::FONT_MONO)
                            .text_color(theme::text_subtle())
                            .truncate()
                            .child(item.parent_dir.clone()),
                    ),
            )
    }
}
