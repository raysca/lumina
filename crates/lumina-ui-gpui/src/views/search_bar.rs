use crate::theme;
use crate::LuminaApp;
use gpui::*;

pub struct SearchBarView;

impl SearchBarView {
    /// Search field that lives in the toolbar.
    pub fn render_field(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let active = !this.search_query.is_empty();
        let focused = this.search_focused;
        let query_text = if focused {
            format!("{}|", this.search_query)
        } else if active {
            this.search_query.clone()
        } else {
            "Search — try camera:sony iso:>3200".to_string()
        };

        div()
            .id("search-input-box")
            .w(px(280.0))
            .h(px(28.0))
            .px_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .rounded(px(6.0))
            .border_1()
            .border_color(if focused || active {
                theme::accent()
            } else {
                theme::border()
            })
            .bg(theme::bg_viewer())
            .cursor_text()
            .text_xs()
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.search_focused = true;
                cx.notify();
            }))
            .child(crate::icons::render_icon(
                crate::icons::Icon::Search,
                px(13.0),
                theme::text_subtle(),
            ))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_color(if focused || active {
                        theme::text()
                    } else {
                        theme::text_subtle()
                    })
                    .child(query_text),
            )
            .children(active.then(|| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        div()
                            .text_color(theme::text_muted())
                            .child(format!("{}", this.get_active_count())),
                    )
                    .child(
                        div()
                            .id("clear-search-btn")
                            .w(px(16.0))
                            .h(px(16.0))
                            .rounded_full()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .flex()
                            .justify_center()
                            .items_center()
                            .cursor_pointer()
                            .text_color(theme::text_muted())
                            .child(crate::icons::render_icon(
                                crate::icons::Icon::X,
                                px(10.0),
                                theme::text_muted(),
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.clear_search(cx);
                            })),
                    )
            }))
    }

    /// Filter bar under the toolbar: quick filters, view fit/square toggle, zoom slider.
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let is_fav = this.search_query.contains("is:favorite");
        let is_video = this.search_query.contains("is:video");
        let is_raw = this.search_query.contains("is:raw");
        let is_gps = this.search_query.contains("has:location");
        let is_tagged = this.search_query.contains("has:tag");
        let is_all = !is_fav && !is_video && !is_raw && !is_gps && !is_tagged;

        div()
            .w_full()
            .h(px(44.0))
            .flex_shrink_0()
            .px_4()
            .bg(theme::bg_pane())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            // 1. All
            .child(
                theme::chip("filter-chip-all", is_all)
                    .child("All")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.clear_search(cx);
                    })),
            )
            // 2. Favorites
            .child(
                theme::chip("filter-chip-favs", is_fav)
                    .child("Favorites")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        if this.search_query.contains("is:favorite") {
                            let rest: Vec<&str> = this
                                .search_query
                                .split_whitespace()
                                .filter(|t| *t != "is:favorite")
                                .collect();
                            this.set_search_query(&rest.join(" "), cx);
                        } else {
                            this.append_search_pill("is:favorite", cx);
                        }
                    })),
            )
            // 3. Videos
            .child(
                theme::chip("filter-chip-videos", is_video)
                    .child("Videos")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        if this.search_query.contains("is:video") {
                            let rest: Vec<&str> = this
                                .search_query
                                .split_whitespace()
                                .filter(|t| *t != "is:video")
                                .collect();
                            this.set_search_query(&rest.join(" "), cx);
                        } else {
                            this.append_search_pill("is:video", cx);
                        }
                    })),
            )
            // 4. RAW
            .child(
                theme::chip("filter-chip-raw", is_raw)
                    .child("RAW")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        if this.search_query.contains("is:raw") {
                            let rest: Vec<&str> = this
                                .search_query
                                .split_whitespace()
                                .filter(|t| *t != "is:raw")
                                .collect();
                            this.set_search_query(&rest.join(" "), cx);
                        } else {
                            this.append_search_pill("is:raw", cx);
                        }
                    })),
            )
            // 5. Has GPS
            .child(
                theme::chip("filter-chip-gps", is_gps)
                    .child("Places")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        if this.search_query.contains("has:location") {
                            let rest: Vec<&str> = this
                                .search_query
                                .split_whitespace()
                                .filter(|t| *t != "has:location")
                                .collect();
                            this.set_search_query(&rest.join(" "), cx);
                        } else {
                            this.append_search_pill("has:location", cx);
                        }
                    })),
            )
            // 6. Tagged
            .child(
                theme::chip("filter-chip-tagged", is_tagged)
                    .child("Tagged")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        if this.search_query.contains("has:tag") {
                            let rest: Vec<&str> = this
                                .search_query
                                .split_whitespace()
                                .filter(|t| *t != "has:tag")
                                .collect();
                            this.set_search_query(&rest.join(" "), cx);
                        } else {
                            this.append_search_pill("has:tag", cx);
                        }
                    })),
            )
            .child(div().flex_1())
            // [ Fit | Square ] segmented toggle
            .child(
                theme::segmented("fit-square-toggle")
                    .child(
                        theme::segmented_item("fit-mode-fit", this.card_fit_mode)
                            .child("Fit")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.card_fit_mode = true;
                                this.save_config();
                                cx.notify();
                            })),
                    )
                    .child(
                        theme::segmented_item("fit-mode-square", !this.card_fit_mode)
                            .child("Square")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.card_fit_mode = false;
                                this.save_config();
                                cx.notify();
                            })),
                    ),
            )
            // Zoom slider control
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(
                        theme::control("grid-zoom-out-btn")
                            .w(px(22.0))
                            .h(px(22.0))
                            .px_0()
                            .justify_center()
                            .child("−")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.zoom_out_grid(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("zoom-slider-track")
                            .w(px(64.0))
                            .h(px(12.0))
                            .flex()
                            .flex_row()
                            .items_center()
                            .cursor_pointer()
                            .relative()
                            // Visual track line
                            .child(div().w_full().h(px(4.0)).rounded_full().bg(theme::border()))
                            // Thumb
                            .child({
                                let cols = this.grid_columns.clamp(2, 8);
                                let frac = ((cols - 2) as f32 / 6.0).clamp(0.0, 1.0);
                                let left_offset = (1.0 - frac) * 52.0;
                                div()
                                    .absolute()
                                    .top(px(0.0))
                                    .left(px(left_offset))
                                    .size(px(12.0))
                                    .rounded_full()
                                    .bg(theme::accent())
                            })
                            // Interactive click steps (8 cols on left -> 2 cols on right)
                            .child(div().absolute().inset_0().flex().flex_row().children(
                                (2usize..=8).rev().map(|target_cols| {
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "zoom-slider-step".into(),
                                            target_cols as u64,
                                        ))
                                        .flex_1()
                                        .h_full()
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.grid_columns = target_cols;
                                                this.save_config();
                                                cx.notify();
                                            },
                                        ))
                                }),
                            )),
                    )
                    .child(
                        theme::control("grid-zoom-in-btn")
                            .w(px(22.0))
                            .h(px(22.0))
                            .px_0()
                            .justify_center()
                            .child("+")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.zoom_in_grid(cx);
                            })),
                    ),
            )
    }
}
