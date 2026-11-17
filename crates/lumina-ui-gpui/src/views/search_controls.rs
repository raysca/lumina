//! The search header's controls beside the query: the
//! Save as Smart Album button and the Narrow by chips.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme::{self, ChipKind};

/// The Narrow by chips, in the frame's order: selector
/// suffix, the quick-filter token each toggles, icon, label key.
const NARROW_CHIPS: [(&str, &str, Icon, &str); 5] = [
    ("fnumber", "f:<2", Icon::Aperture, "search.narrow_fnumber"),
    ("video", "is:video", Icon::Video, "search.narrow_video"),
    ("rating", "rating:>=4", Icon::Star, "search.narrow_rating"),
    ("edited", "is:edited", Icon::Pencil, "search.narrow_edited"),
    ("raw", "is:raw", Icon::FileImage, "search.narrow_raw"),
];

impl LuminaApp {
    /// Whether Save as Smart Album can run: a query to save, and no compare
    /// or edit mode holding the view. Zero results still saves.
    pub(crate) fn search_save_enabled(&self) -> bool {
        !self.search_query.trim().is_empty()
            && !self.compare_holds_view()
            && self.edits.session.is_none()
    }

    /// "Narrow by" and its chips; each toggles its token in the query, as
    /// the palette's quick filters do. Scrolls sideways in a narrow window.
    pub(crate) fn render_narrow_row(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let active: Vec<lumina_core::StructuredQuery> =
            lumina_core::search::tokenize_query(&self.search_query)
                .iter()
                .map(|token| lumina_core::StructuredQuery::parse(token))
                .collect();
        div()
            .id("search-narrow")
            .debug_selector(|| "search-narrow".to_string())
            .role(Role::Toolbar)
            .aria_label(SharedString::from(t("search.narrow_by")))
            .w_full()
            .h(px(28.0))
            .flex_shrink_0()
            .overflow_x_scroll()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(11.0))
                    .text_color(theme::t3())
                    .child(t("search.narrow_by")),
            )
            .children(NARROW_CHIPS.iter().map(|&(name, token, icon, key)| {
                let on = active.contains(&lumina_core::StructuredQuery::parse(token));
                let label = t(key);
                let ink = if on { theme::accent() } else { theme::t2() };
                theme::chip(
                    SharedString::from(format!("search-narrow-{name}")),
                    ChipKind::Ghost,
                )
                .debug_selector(move || format!("search-narrow-{name}"))
                .role(Role::Button)
                .aria_label(SharedString::from(label.clone()))
                .aria_toggled(if on { Toggled::True } else { Toggled::False })
                .flex_shrink_0()
                .cursor_pointer()
                .when(on, |chip| {
                    chip.bg(theme::accent_soft())
                        .border_color(theme::accent())
                        .text_color(theme::accent())
                })
                .when(!on, |chip| chip.hover(|s| s.bg(theme::bg_2())))
                .child(render_icon(icon, px(13.0), ink))
                .child(
                    div()
                        .when(on, |d| {
                            d.debug_selector(move || format!("search-narrow-{name}-on"))
                        })
                        .child(label),
                )
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.toggle_quick_filter(token, cx);
                }))
            }))
    }

    /// Save as Smart Album: the palette's Save Search, as a button. Off
    /// while compare or edit mode holds the view.
    pub(crate) fn render_save_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = self.search_save_enabled();
        let label = t("command.save_search").trim_end_matches('…').to_string();
        div()
            .id("search-save-smart-album")
            .debug_selector(|| "search-save-smart-album".to_string())
            .role(Role::Button)
            .aria_label(SharedString::from(label.clone()))
            .when(!enabled, |b| {
                b.aria_description(SharedString::from(t("search.save_unavailable")))
            })
            .tooltip(theme::tooltip_label(if enabled {
                t("command.save_search")
            } else {
                t("search.save_unavailable")
            }))
            .h(px(28.0))
            .px(px(10.0))
            .flex_shrink_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(theme::line_2())
            .bg(theme::bg_3())
            .text_size(px(11.5))
            .text_color(theme::t1())
            .child(render_icon(Icon::Sparkles, px(12.0), theme::t1()))
            .child(label)
            .when(enabled, |b| {
                b.cursor_pointer()
                    .hover(|s| s.border_color(theme::accent()))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        cx.stop_propagation();
                        // The bar keeps search focused through the click;
                        // the builder takes the keys, and the dropdown must
                        // not paint over it.
                        this.blur_search(cx);
                        this.save_current_search_as_album(cx);
                    }))
            })
            .when(!enabled, |b| b.opacity(0.5))
    }
}
