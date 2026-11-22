//! The Photos section head: in place of the
//! month band while the search header shows, pinned above the grid like
//! the band and the Top matches block. "Photos" (or "Videos", or "Photos &
//! Videos" for a mix), the result count, and the current sort, which opens
//! the sort menu under itself.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::theme;
use crate::views::grid::{
    BAND_TO_ROW, CARD_ROW_GAP, GRID_ROW_PADDING_LEFT, GRID_ROW_PADDING_RIGHT,
};
use crate::views::page_header::popover_under;

impl LuminaApp {
    /// The head's title and count. The kinds come from the query when it
    /// names one (`is:video`, `is:photo`, `is:raw`), else from the view
    /// fetch's video count; "Photos" until that lands.
    pub(crate) fn results_head(&self) -> (String, String) {
        let count = self.displayed_count();
        let named = lumina_core::StructuredQuery::parse(&self.search_query).media_type;
        let key = match (named.as_deref(), self.displayed_videos()) {
            (Some("video"), _) => "search.results_videos",
            (Some(_), _) => "search.results_photos",
            (None, Some(videos)) if videos > 0 && videos < count => "search.results_mixed",
            (None, Some(videos)) if videos > 0 => "search.results_videos",
            _ => "search.results_photos",
        };
        (t(key), crate::i18n::plural("header.items", count))
    }

    /// The current sort field and ▾ ("Date taken ▾"); results have no
    /// relevance order, so there is no "Best match".
    pub(crate) fn results_sort_label(&self) -> String {
        format!("{} ▾", crate::app::sort::sort_field_name(self.sort.field))
    }

    /// The head, the month band's size and spacing so the grid below sits
    /// where it did.
    pub(crate) fn render_results_head(&self, cx: &mut Context<Self>) -> AnyElement {
        let (title, count) = self.results_head();
        let sort = self.results_sort_label();
        div()
            .debug_selector(|| "search-results-head".to_string())
            .flex_none()
            .h(px(36.0))
            .pl(px(GRID_ROW_PADDING_LEFT))
            .pr(px(GRID_ROW_PADDING_RIGHT))
            .mb(px(BAND_TO_ROW - CARD_ROW_GAP))
            .flex()
            .flex_row()
            .items_end()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(9.0))
                    // ponytail: the design's -0.2 tracking is skipped; GPUI
                    // 0.3.11 has no letter-spacing style.
                    .child(
                        div()
                            .debug_selector(|| "search-results-title".to_string())
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(theme::TEXT_LARGE)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .child(title),
                    )
                    .child(
                        div()
                            .debug_selector(|| "search-results-count".to_string())
                            .font_family(theme::FONT_SANS)
                            .font_features(theme::tabular_figures())
                            .text_size(theme::TEXT_CAPTION)
                            .text_color(theme::t3())
                            .child(count),
                    ),
            )
            .child(
                div()
                    .id("search-results-sort")
                    .debug_selector(|| "search-results-sort".to_string())
                    .role(Role::Button)
                    .aria_expanded(self.sort_menu_open)
                    .aria_label(SharedString::from(t("search.results_sort").replace(
                        "{sort}",
                        &crate::app::sort::sort_field_name(self.sort.field),
                    )))
                    .relative()
                    .cursor_pointer()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme::t2())
                    .hover(|s| s.text_color(theme::t1()))
                    .child(sort)
                    .when(self.sort_menu_open, |label| {
                        label.child(popover_under(
                            Anchor::TopRight,
                            crate::views::sort_menu::sort_menu_card(self, cx),
                        ))
                    })
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sort_menu(cx);
                    })),
            )
            .into_any_element()
    }
}
