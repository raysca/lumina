use gpui::*;
use lumina_core::MediaItem;
use std::cmp;

use super::utils::render_shortcut_hint;
use crate::app::LuminaApp;
use crate::theme;

pub struct ViewerFilmstrip;

impl ViewerFilmstrip {
    pub fn render(
        this: &LuminaApp,
        item: &MediaItem,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let total_count = this.get_active_count();
        let selected_idx = this.selected_index.unwrap_or(0);

        // Fetch surrounding window of items centered around current selection
        let (items, start_idx): (Vec<MediaItem>, usize) = if total_count > 0 {
            let half_window = 4;
            let start = selected_idx.saturating_sub(half_window);
            let end = cmp::min(total_count, start + 8);
            let actual_start = if end - start < 8 && total_count >= 8 {
                end.saturating_sub(8)
            } else {
                start
            };
            (this.get_active_slice(actual_start..end), actual_start)
        } else {
            (Vec::new(), 0)
        };

        let cur_id = item.id;

        div()
            .id("viewer-bottom-tray")
            .w_full()
            .h(px(64.0))
            .bg(theme::bg_app())
            .border_t_1()
            .border_color(theme::border_subtle())
            .px_4()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            // Left: Album filmstrip thumbnails
            .child(div().flex().flex_row().items_center().gap_2().children(
                items.into_iter().enumerate().map(|(rel_idx, it)| {
                    let abs_idx = start_idx + rel_idx;
                    let is_sel = it.id == cur_id;
                    let it_clone = it.clone();

                    let texture = if let Some(ref state) = this.grid_state {
                        let mut cache = state.texture_cache.lock();
                        if let Some(tex) = cache.get(&it.id).cloned() {
                            Some(tex)
                        } else {
                            drop(cache);
                            this.queue_thumbnail_request(it.id);
                            None
                        }
                    } else {
                        None
                    };

                    div()
                        .id(ElementId::NamedInteger("bottom-tray-thumb".into(), it.id.0))
                        .w(px(68.0))
                        .h(px(46.0))
                        .rounded_sm()
                        .overflow_hidden()
                        .cursor_pointer()
                        .border_2()
                        .border_color(if is_sel {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .bg(if is_sel {
                            theme::selection_bg()
                        } else {
                            theme::bg_pane()
                        })
                        .hover(|s| s.border_color(theme::accent()))
                        .child(if let Some(tex) = texture {
                            div()
                                .size_full()
                                .child(img(tex).size_full().object_fit(ObjectFit::Cover))
                                .into_any_element()
                        } else {
                            div().size_full().bg(theme::bg_pane()).into_any_element()
                        })
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.select_index(abs_idx, cx);
                            this.open_viewer(it_clone.clone(), cx);
                        }))
                }),
            ))
            // Right: Keyboard shortcuts helper legend
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .child(render_shortcut_hint(vec!["Space"], "play"))
                    .child(render_shortcut_hint(vec!["J", "K", "L"], "shuttle"))
                    .child(render_shortcut_hint(vec![",", "."], "frame"))
                    .child(render_shortcut_hint(vec!["←", "→"], "5 s"))
                    .child(render_shortcut_hint(vec!["M"], "mute"))
                    .child(render_shortcut_hint(vec!["⇧L"], "loop"))
                    .child(render_shortcut_hint(vec!["⌥P"], "PiP"))
                    .child(render_shortcut_hint(vec!["Z"], "zoom")),
            )
    }
}
