use gpui::*;
use crate::LuminaApp;

pub struct SearchBarView;

impl SearchBarView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let is_placeholder = this.search_query.is_empty();
        let query_text = if is_placeholder {
            "Search photos, gear, location (e.g. camera:Sony, is:favorite, year:2026)...".to_string()
        } else {
            this.search_query.clone()
        };

        let result_count = if !this.search_query.is_empty() {
            Some(this.get_active_count())
        } else {
            None
        };

        let timeline_buckets = this
            .grid_state
            .as_ref()
            .map(|s| s.engine.get_timeline_buckets("year"))
            .unwrap_or_default();

        div()
            .w_full()
            .bg(rgb(0x121215))
            .border_b_1()
            .border_color(rgb(0x27272a))
            .flex()
            .flex_col()
            .px_4()
            .py_2()
            .gap_2()
            // Search Input Row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("search-input-box")
                            .flex_1()
                            .h(px(34.0))
                            .px_3()
                            .bg(rgb(0x18181b))
                            .border_1()
                            .border_color(if !this.search_query.is_empty() { rgb(0x3b82f6) } else { rgb(0x27272a) })
                            .rounded_md()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x71717a))
                                            .child("🔍"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(if is_placeholder { rgb(0x71717a) } else { rgb(0xffffff) })
                                            .font_weight(if is_placeholder { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                                            .child(query_text),
                                    ),
                            )
                            .child(if !this.search_query.is_empty() {
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(if let Some(count) = result_count {
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_full()
                                            .bg(rgb(0x1e293b))
                                            .text_xs()
                                            .text_color(rgb(0x38bdf8))
                                            .child(format!("{} results (< 1ms)", count))
                                            .into_any_element()
                                    } else {
                                        Empty.into_any_element()
                                    })
                                    .child(
                                        div()
                                            .id("clear-search-btn")
                                            .w(px(18.0))
                                            .h(px(18.0))
                                            .rounded_full()
                                            .bg(rgb(0x27272a))
                                            .hover(|s| s.bg(rgb(0x3f3f46)))
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(rgb(0xa1a1aa))
                                            .child("✕")
                                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                                this.clear_search(cx);
                                            })),
                                    )
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    ),
            )
            // Suggestion Pills & Timeline Scrubber
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    // Suggestion Pills
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x71717a))
                                    .child("Filters:"),
                            )
                            .child(render_suggestion_pill(
                                "is:favorite",
                                "♥ Favorite",
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.append_search_pill("is:favorite", cx);
                                }),
                            ))
                            .child(render_suggestion_pill(
                                "is:video",
                                "▶ Video",
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.append_search_pill("is:video", cx);
                                }),
                            ))
                            .child(render_suggestion_pill(
                                "is:raw",
                                "📷 RAW",
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.append_search_pill("is:raw", cx);
                                }),
                            ))
                            .child(render_suggestion_pill(
                                "camera:Sony",
                                "Sony",
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.append_search_pill("camera:Sony", cx);
                                }),
                            ))
                            .child(render_suggestion_pill(
                                "has:location",
                                "📍 GPS",
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.append_search_pill("has:location", cx);
                                }),
                            )),
                    )
                    // Timeline Scrubber
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x71717a))
                                    .child("Timeline:"),
                            )
                            .child(
                                div()
                                    .id("timeline-bucket-all")
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(if this.active_year.is_none() { rgb(0x2563eb) } else { rgb(0x18181b) })
                                    .hover(|s| s.bg(rgb(0x27272a)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(if this.active_year.is_none() { rgb(0xffffff) } else { rgb(0xa1a1aa) })
                                    .child("All")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.set_active_year(None, cx);
                                    })),
                            )
                            .children(timeline_buckets.into_iter().enumerate().map(|(ix, bucket)| {
                                let year: i32 = bucket.bucket_key.parse().unwrap_or(2026);
                                let is_active = this.active_year == Some(year);
                                let label = format!("{} ({})", bucket.bucket_key, bucket.item_count);
                                div()
                                    .id(ElementId::NamedInteger("timeline-bucket-year".into(), ix as u64))
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(if is_active { rgb(0x2563eb) } else { rgb(0x18181b) })
                                    .hover(|s| s.bg(rgb(0x27272a)))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(if is_active { rgb(0xffffff) } else { rgb(0xa1a1aa) })
                                    .child(label)
                                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                        this.set_active_year(Some(year), cx);
                                    }))
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn render_suggestion_pill(
    _token: &'static str,
    label: &'static str,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(ElementId::Name(label.into()))
        .px_2()
        .py_0p5()
        .rounded_md()
        .bg(rgb(0x18181b))
        .border_1()
        .border_color(rgb(0x27272a))
        .hover(|s| s.bg(rgb(0x27272a)).border_color(rgb(0x3b82f6)))
        .cursor_pointer()
        .text_xs()
        .text_color(rgb(0xd4d4d8))
        .child(label)
        .on_click(on_click)
}
