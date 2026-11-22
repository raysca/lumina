//! The sidebar with no library open (the first-run state):
//! the live shell, dimmed, with nothing to click, focus or search.

use gpui::*;

use super::nav::nav_entries;
use crate::i18n::t;
use crate::theme;
use crate::LuminaApp;

/// The dormant sidebar: a search placeholder, All Media / Favorites /
/// Videos with a count of 0, and a hint under each section label. Rows keep
/// their live `sidebar-nav-{key}` names but have no click handler. It
/// ignores `sidebar_collapsed` on purpose: there is no dormant rail.
pub(crate) fn render_dormant(app: &LuminaApp) -> Div {
    div()
        .debug_selector(|| "sidebar".to_string())
        .w(px(app.sidebar_width))
        .h_full()
        .flex_none()
        .bg(theme::bg_1())
        .border_r_1()
        .border_color(theme::line())
        .flex()
        .flex_col()
        .gap(px(super::SIDEBAR_GAP))
        .pb(px(16.0))
        .px(px(super::SIDEBAR_PADDING_X))
        .child(super::render_brand("sidebar-brand", None))
        .child(
            div()
                .debug_selector(|| "sidebar-dormant-search".to_string())
                .opacity(0.5)
                .h(px(34.0))
                .px(px(10.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .rounded(theme::RADIUS_LG)
                .border_1()
                .border_color(theme::line())
                .bg(theme::bg_0())
                .text_size(px(12.5))
                .text_color(theme::t3())
                .child(
                    crate::icons::render_icon(crate::icons::Icon::Search, px(14.0), theme::t3())
                        .flex_none(),
                )
                .child(t("sidebar.dormant_search")),
        )
        .child(
            div().flex().flex_col().gap(px(3.0)).children(
                nav_entries(app)
                    .into_iter()
                    .filter(|entry| matches!(entry.key, "all" | "favorites" | "videos"))
                    .map(|entry| {
                        let key = entry.key;
                        theme::nav_row(
                            format!("sidebar-nav-{key}"),
                            entry.icon,
                            entry.label,
                            0,
                            false,
                            false,
                            false,
                        )
                        // ponytail: GPUI 0.3.11 has no aria disabled; the
                        // description says the row is unavailable. Swap to
                        // `aria_disabled` once GPUI has it.
                        .aria_description(t("sidebar.unavailable"))
                        .opacity(0.45)
                        .children(crate::views::page_header::text_marker(
                            "sidebar-nav-dormant",
                            &key.into(),
                        ))
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(10.0))
                                .font_family(theme::FONT_MONO)
                                .text_color(theme::t3())
                                .child(crate::i18n::count(0)),
                        )
                    }),
            ),
        )
        .child(hint(t("sidebar.albums"), t("sidebar.dormant_albums")))
        .child(hint(t("sidebar.smart_albums"), t("sidebar.dormant_smart")))
        .child(hint(t("sidebar.tags"), t("sidebar.dormant_tags")))
}

/// A section label over a 34px outlined box holding `text`.
fn hint(label: String, text: String) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.0))
        .child(div().px(px(10.0)).child(theme::section_label(label)))
        .child(
            div()
                .h(px(34.0))
                .px(px(10.0))
                .flex()
                .items_center()
                .rounded(px(7.0))
                .border_1()
                .border_color(theme::line())
                .text_size(px(11.0))
                .text_color(theme::t3())
                .child(text),
        )
}
