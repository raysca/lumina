use gpui::*;

use crate::app::LuminaApp;
use crate::theme;

impl LuminaApp {
    pub(crate) fn render_year_rail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.timeline_year_buckets.is_empty() {
            return Empty.into_any_element();
        }

        let years: Vec<i32> = self.timeline_year_buckets.iter().map(|(y, _)| *y).collect();

        div()
            .w(px(crate::views::grid::YEAR_RAIL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgb(0x222326))
            .flex()
            .flex_col()
            .items_center()
            .pt_3()
            .gap(px(18.0))
            .children(years.into_iter().map(|year| {
                let short_year = format!("'{:02}", year.rem_euclid(100));
                let is_active = self.active_year == Some(year);
                div()
                    .id(ElementId::NamedInteger(
                        "year-rail-item".into(),
                        year as u64,
                    ))
                    .px_1p5()
                    .py_0p5()
                    .rounded_sm()
                    .bg(if is_active {
                        theme::accent()
                    } else {
                        rgba(0x00000000)
                    })
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .font_family(theme::FONT_MONO)
                    .font_weight(if is_active {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(if is_active {
                        theme::accent_fg()
                    } else {
                        theme::text_subtle()
                    })
                    .hover(move |s| {
                        if !is_active {
                            s.text_color(theme::text())
                        } else {
                            s
                        }
                    })
                    .child(short_year)
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        if this.active_year == Some(year) {
                            this.set_active_year(None, cx);
                        } else {
                            this.set_active_year(Some(year), cx);
                        }
                    }))
            }))
            .into_any_element()
    }
}
