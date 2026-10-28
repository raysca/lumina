use std::cmp;

use gpui::*;
use lumina_core::MediaType;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

impl LuminaApp {
    pub(crate) fn render_filmstrip_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_count = self.get_active_count();
        let items = if active_count > 0 {
            self.get_active_slice(0..cmp::min(active_count, 300))
        } else {
            Vec::new()
        };

        div()
            .id("workspace-filmstrip-bar")
            .h(px(88.0))
            .w_full()
            .bg(theme::bg_app())
            .border_t_1()
            .border_color(theme::border())
            .px_3()
            .py_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .overflow_x_scroll()
            .children(items.into_iter().enumerate().map(|(idx, item)| {
                let is_selected = self
                    .selected_item
                    .as_ref()
                    .map(|it| it.id == item.id)
                    .unwrap_or(false);
                let item_clone = item.clone();
                let is_video = item.media_type == MediaType::Video;
                let texture = if let Some(ref state) = self.grid_state {
                    let cached = state.texture_cache.lock().get(&item.id).cloned();
                    if cached.is_none() {
                        self.queue_thumbnail_request(item.id);
                    }
                    cached
                } else {
                    None
                };

                div()
                    .id(ElementId::NamedInteger(
                        "filmstrip-thumb".into(),
                        idx as u64,
                    ))
                    .h(px(70.0))
                    .w(px(90.0))
                    .flex_shrink_0()
                    .rounded_md()
                    .overflow_hidden()
                    .cursor_pointer()
                    .border_2()
                    .border_color(if is_selected {
                        theme::accent()
                    } else {
                        theme::border_subtle()
                    })
                    .bg(if is_selected {
                        theme::selection_bg()
                    } else {
                        theme::bg_pane()
                    })
                    .hover(|s| s.border_color(theme::accent()))
                    .relative()
                    .child(if let Some(tex) = texture {
                        div()
                            .size_full()
                            .child(img(tex).size_full().object_fit(ObjectFit::Cover))
                            .into_any_element()
                    } else {
                        div()
                            .size_full()
                            .bg(theme::bg_control())
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_xs()
                            .font_family(theme::FONT_MONO)
                            .text_color(theme::text_subtle())
                            .child(render_icon(
                                if is_video { Icon::Play } else { Icon::Image },
                                px(14.0),
                                theme::text_subtle(),
                            ))
                            .into_any_element()
                    })
                    .children(is_selected.then(|| {
                        div()
                            .absolute()
                            .top_1()
                            .right_1()
                            .size(px(6.0))
                            .rounded_full()
                            .bg(theme::accent())
                    }))
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.select_item_at(item_clone.clone(), idx, cx);
                    }))
            }))
    }
}
