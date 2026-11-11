//! The Videos chip row (frame `DoXZy`): "All clips" and "4K", then the clip
//! count and Clear all. HDR, slow motion and Live Photos chips stay out
//! until the catalog stores that data (Design 04 decision, 2026-11-11).

use gpui::prelude::FluentBuilder;
use gpui::*;

use lumina_core::{StructuredQuery, SystemAlbum};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::theme::{self, ChipKind};
use crate::views::sidebar::AlbumSelection;

/// The search the 4K chip sets. A search spans the whole library
/// (`docs/search.md`, Scope), so `is:video` keeps 4K photos out.
pub(crate) const VIDEOS_4K_QUERY: &str = "is:video is:4k";

/// Whether `query` is the 4K chip's search, in any token order or alias.
pub(crate) fn is_videos_4k_query(query: &str) -> bool {
    StructuredQuery::parse(query) == StructuredQuery::parse(VIDEOS_4K_QUERY)
}

/// The Videos view: Videos selected with no search or year, or the 4K
/// chip's search (which, like every search, moves the selection to the
/// library). Its counts read "clips" and its header shows the totals.
pub(crate) fn is_videos_scope(selection: &AlbumSelection, query: &str, year: Option<i32>) -> bool {
    year.is_none()
        && ((query.is_empty() && *selection == AlbumSelection::System(SystemAlbum::Videos))
            || is_videos_4k_query(query))
}

impl LuminaApp {
    /// Whether the 4K chip is on.
    pub(crate) fn videos_4k_active(&self) -> bool {
        self.active_year.is_none() && is_videos_4k_query(&self.search_query)
    }

    /// The 4K chip and Filter: 4K Videos: sets the 4K search, or returns
    /// to all clips when it is already on.
    pub(crate) fn toggle_videos_4k(&mut self, cx: &mut Context<Self>) {
        if self.videos_4k_active() {
            self.show_all_clips(cx);
        } else {
            self.set_search_query(VIDEOS_4K_QUERY, cx);
        }
    }

    /// All clips and Clear all: plain Videos, with no search.
    pub(crate) fn show_all_clips(&mut self, cx: &mut Context<Self>) {
        if self.videos_4k_active() {
            self.select_album(AlbumSelection::System(SystemAlbum::Videos), cx);
        }
    }

    /// The empty state's title and body in Videos, as clips; None outside
    /// Videos and for an empty library (whose own copy says so).
    pub(crate) fn videos_empty_copy(&self) -> Option<(String, String)> {
        if self.videos_4k_active() {
            Some((t("videos.empty_4k_title"), t("videos.empty_4k_body")))
        } else if self.videos_view() && self.total_library_count > 0 {
            Some((t("videos.empty_title"), t("videos.empty_body")))
        } else {
            None
        }
    }

    /// The Videos chip row, in place of the Library's: "All clips" and
    /// "4K" toggle buttons, the clip count, and Clear all. Shown with no
    /// filter too, so the chips are always there to pick.
    pub(crate) fn render_videos_filter_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let uhd = self.videos_4k_active();
        let chip = |name: &'static str, label: String, on: bool| {
            theme::chip(
                SharedString::from(format!("filter-chip-{name}")),
                ChipKind::Normal,
            )
            .debug_selector(move || format!("filter-chip-{name}"))
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
            .when(!on, |chip| chip.hover(|s| s.bg(theme::bg_3())))
            .child(label)
        };
        div()
            .id("filter-bar")
            .debug_selector(|| "filter-bar".to_string())
            .role(Role::Toolbar)
            .aria_label(SharedString::from(t("header.filter")))
            .w_full()
            .h(px(51.5))
            .flex_shrink_0()
            .px(px(28.0))
            .border_b_1()
            .border_color(theme::line())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        chip("all", t("filter.all_clips"), !uhd).on_click(cx.listener(
                            |this, _e: &ClickEvent, _w, cx| {
                                this.show_all_clips(cx);
                            },
                        )),
                    )
                    .child(chip("4k", t("filter.4k"), uhd).on_click(cx.listener(
                        |this, _e: &ClickEvent, _w, cx| {
                            this.toggle_videos_4k(cx);
                        },
                    ))),
            )
            .children(self.last_fetched_count().map(|n| {
                let label = self.count_label(n);
                div()
                    .id("filter-match-count")
                    .debug_selector(|| "filter-match-count".to_string())
                    .role(Role::Status)
                    .aria_label(SharedString::from(label.clone()))
                    .flex_shrink_0()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(10.5))
                    .text_color(theme::t3())
                    .child(label)
            }))
            .child(
                div()
                    .id("filter-clear-all")
                    .debug_selector(|| "filter-clear-all".to_string())
                    .role(Role::Button)
                    .aria_label(SharedString::from(t("filter.clear_all")))
                    .flex_shrink_0()
                    .cursor_pointer()
                    .text_size(px(11.5))
                    .text_color(theme::accent())
                    .hover(|s| s.text_color(theme::accent_hover()))
                    .child(t("filter.clear_all"))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.show_all_clips(cx);
                    })),
            )
    }
}
