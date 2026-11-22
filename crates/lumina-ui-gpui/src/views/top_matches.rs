//! The TOP MATCHES block: under the
//! search header, above the grid, so the header keeps its 131.5. Cards are
//! four to a row, two below 1100 px of content width.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::search::{TopMatch, TopMatchKind};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// Content width below which the cards drop to two per row.
const TWO_COLUMNS_BELOW: f32 = 1100.0;

impl LuminaApp {
    /// The block, or None when nothing matches (the row hides).
    pub(crate) fn render_top_matches(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let matches = self.top_matches();
        if matches.is_empty() {
            return None;
        }
        let pane = crate::views::grid::center_pane_width(
            window.viewport_size().width.into(),
            self.sidebar_width,
            self.sidebar_collapsed,
            false,
            self.inspector_open.then_some(self.inspector_width),
        );
        let columns = if pane - 56.0 < TWO_COLUMNS_BELOW {
            2
        } else {
            4
        };
        let highlight = self.top_matches.highlight;
        let mut seen = [0usize; 5];
        let cards: Vec<Stateful<Div>> = matches
            .iter()
            .enumerate()
            .map(|(i, found)| {
                let stem = found.kind.selector();
                let slot = match stem {
                    "place" => 0,
                    "album" => 1,
                    "month" => 2,
                    "year" => 3,
                    _ => 4,
                };
                let n = seen[slot];
                seen[slot] += 1;
                card(i, stem, n, found, highlight == Some(i), cx)
            })
            .collect();
        Some(
            div()
                .id("top-matches")
                .debug_selector(|| "top-matches".to_string())
                .role(Role::Group)
                .aria_label(SharedString::from(t("search.top_matches")))
                .w_full()
                .flex_shrink_0()
                .px(px(28.0))
                .pt(px(22.0))
                .flex()
                .flex_col()
                .gap(px(11.0))
                .child(
                    div()
                        .text_size(theme::TEXT_CAPTION)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t3())
                        // The label is stored in sentence case and drawn in capitals.
                        .child(t("search.top_matches").to_uppercase()),
                )
                .child(
                    div()
                        .w_full()
                        .grid()
                        .grid_cols(columns)
                        .gap(px(10.0))
                        .children(cards),
                ),
        )
    }
}

/// One card: 64 high, 36 circle with the kind's icon, title, "Kind · n
/// items" and a chevron. `n` numbers the cards of one kind.
fn card(
    i: usize,
    stem: &'static str,
    n: usize,
    found: &TopMatch,
    highlighted: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let (icon, kind_key) = match found.kind {
        TopMatchKind::Place { .. } => (Icon::MapPin, "search.match_place"),
        TopMatchKind::Album(_) => (Icon::Sparkles, "search.match_album"),
        TopMatchKind::Moment { .. } => (Icon::Calendar, "search.match_moment"),
        TopMatchKind::Tag(_) => (Icon::Tag, "search.match_tag"),
    };
    let kind = t(kind_key);
    let items = crate::i18n::plural("header.items", found.count);
    let label = t("search.match_label")
        .replace("{kind}", &kind)
        .replace("{title}", &found.title)
        .replace("{items}", &items);
    let sub = t("search.match_sub")
        .replace("{kind}", &kind)
        .replace("{items}", &items);
    div()
        .id(("top-match", i))
        .debug_selector(move || format!("top-match-{stem}-{n}"))
        .role(Role::Button)
        .aria_label(SharedString::from(label))
        .min_w_0()
        .h(px(64.0))
        .px(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(11.0))
        .bg(theme::bg_1())
        .border_1()
        .border_color(if highlighted {
            theme::accent()
        } else {
            theme::line()
        })
        .rounded(px(9.0))
        .cursor_pointer()
        .hover(|s| s.bg(theme::bg_2()))
        .child(
            div()
                .size(px(36.0))
                .flex_none()
                .rounded_full()
                .bg(theme::bg_3())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(icon, theme::ICON_MD, theme::t2())),
        )
        .child(
            div()
                .when(highlighted, |d| {
                    d.debug_selector(move || format!("top-match-{stem}-{n}-highlight"))
                })
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .truncate()
                        .text_size(theme::TEXT_BODY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .child(found.title.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .font_family(theme::FONT_SANS)
                        .font_features(theme::tabular_figures())
                        .text_size(theme::TEXT_CAPTION)
                        .text_color(theme::t3())
                        .child(sub),
                ),
        )
        .child(render_icon(Icon::ChevronRight, theme::ICON_SM, theme::t3()).flex_none())
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.open_top_match(i, cx);
        }))
}
