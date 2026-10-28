use gpui::*;

use crate::app::LuminaApp;
use crate::theme;

impl LuminaApp {
    pub(crate) fn render_status_bar(&self, total_count: usize) -> impl IntoElement {
        div()
            .h(px(28.0))
            .w_full()
            .bg(theme::bg_app())
            .border_t_1()
            .border_color(theme::border())
            .px_4()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(div().text_xs().text_color(theme::text_subtle()).child({
                let sel_count = if !self.selected_indices.is_empty() {
                    self.selected_indices.len()
                } else if self.selected_item.is_some() {
                    1
                } else {
                    0
                };
                let year_str = if let Some(y) = self.active_year {
                    format!(" · {}", y)
                } else {
                    String::new()
                };
                format!("{} of {} selected{}", sel_count, total_count, year_str)
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_subtle())
                    .child("Right-click any item for actions"),
            )
    }
}
