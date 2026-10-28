pub mod header;
pub mod table;
pub mod toolbars;

use gpui::*;

use crate::theme;
use crate::{LuminaApp, TagSortOrder};
use header::TagManagerHeader;
use table::TagManagerTable;
use toolbars::TagManagerToolbars;

pub struct TagManagerModal;

impl TagManagerModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let total_count = app.library_tag_counts.len();
        let selected_count = app.tag_manager.selected.len();
        let is_merge_active = app.tag_manager.merge_source.is_some();
        let merge_source = app.tag_manager.merge_source.clone().unwrap_or_default();

        // Filter and sort tags
        let query = app.tag_manager.search.to_lowercase();
        let mut filtered_tags: Vec<(String, usize)> = app
            .library_tag_counts
            .iter()
            .filter(|(tag, _)| query.is_empty() || tag.to_lowercase().contains(&query))
            .cloned()
            .collect();

        match app.tag_manager.sort {
            TagSortOrder::Alphabetical => {
                filtered_tags.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
            }
            TagSortOrder::CountDesc => {
                filtered_tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            }
        }

        let filtered_count = filtered_tags.len();

        div()
            .id("tag-manager-backdrop")
            .debug_selector(|| "tag-manager-backdrop".to_string())
            .absolute()
            .inset_0()
            .bg(theme::scrim())
            .flex()
            .justify_center()
            .items_center()
            .p_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_tag_manager(cx);
                }),
            )
            // Clean, Dark Modal Card matching the design system
            .child(
                div()
                    .id("tag-manager-dialog")
                    .debug_selector(|| "tag-manager-dialog".to_string())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .w(px(720.0))
                    .max_h(px(720.0))
                    .bg(theme::bg_pane())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_2xl()
                    .shadow_2xl()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .overflow_hidden()
                    // 1. Title Row: Gold Icon + Title + Count Pill + Close Button
                    .child(TagManagerHeader::render_title(app, total_count, cx))
                    // 2. FILTER BY Presets Row
                    .child(TagManagerHeader::render_presets(app, selected_count, cx))
                    // 3. Search Filter & Sort Row
                    .child(TagManagerHeader::render_controls(app, cx))
                    // 4. Bulk Selection Action Bar
                    .child(TagManagerToolbars::render_bulk_actions(
                        app,
                        selected_count,
                        cx,
                    ))
                    // 5. Active Tag Merge Banner
                    .child(TagManagerToolbars::render_merge_banner(
                        is_merge_active,
                        &merge_source,
                        cx,
                    ))
                    // 6. Scrollable Tags Table
                    .child(TagManagerTable::render_table(
                        app,
                        &filtered_tags,
                        &query,
                        is_merge_active,
                        &merge_source,
                        cx,
                    ))
                    // 7. Bottom Footer: Live Preview Stats + Cancel Esc + Done
                    .child(TagManagerToolbars::render_footer(filtered_count, cx)),
            )
            .into_any_element()
    }
}
