pub mod batch;
pub mod batch_template;
pub mod body;
pub mod summary;
pub mod tabs;

pub use body::{format_bytes, render_inspector_body, InspectorTab};

use crate::theme;
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;
use gpui::*;

pub struct InspectorView;

impl InspectorView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
        let weak_app = cx.entity().downgrade();
        let panel = div()
            .w(px(this.inspector_width))
            .h_full()
            .border_l_1()
            .border_color(theme::line());

        // v3: a single item gets the one-column summary with no header strip;
        // Info / I closes it; ⌘I or Get Info… opens the full tabbed window.
        if this.selected_indices.len() > 1 {
            return panel
                .child(batch::render_batch(this, weak_app))
                .into_any_element();
        }
        let Some(item) = this.selected_item.as_ref() else {
            return panel
                .debug_selector(|| "inspector-empty".to_string())
                .flex_shrink_0()
                .bg(theme::bg_1())
                .p_4()
                .text_sm()
                .text_color(theme::t2())
                .child("Nothing selected.")
                .into_any_element();
        };
        let suggestions = this.tag_input_suggestions();
        panel
            .child(summary::render_summary_body(
                weak_app,
                item,
                &this.selected_item_tags,
                this.tag_input_active,
                this.field_input_for(crate::app::text_input::TextTarget::Tag),
                (&suggestions, this.keywords.suggestion_focus),
                this.selection == AlbumSelection::Trash,
                this.get_item_rating(item),
                this.mark_pop.star(Some(item.id)),
                this.item_color_labels.get(&item.id).copied(),
            ))
            .into_any_element()
    }
}
