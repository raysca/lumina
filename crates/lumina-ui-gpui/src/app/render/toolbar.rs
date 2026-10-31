use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::search_bar::SearchBarView;
use crate::views::sidebar::AlbumSelection;

/// 28×28 borderless icon button; a fill shows on hover, or the selection
/// fill while `on`. The tooltip names the action.
fn icon_button(id: &'static str, on: bool) -> Stateful<Div> {
    if on {
        theme::control_on(id).border_color(rgba(0x00000000))
    } else {
        theme::control_ghost(id)
    }
    .w(px(28.0))
    .flex_shrink_0()
    .px_0()
    .justify_center()
}

/// Toolbar icons: 16px, row text colour, the selection colour while on.
fn toolbar_icon(icon: crate::icons::Icon, on: bool) -> AnyElement {
    let color = if on {
        theme::selection_fg()
    } else {
        theme::text_row()
    };
    crate::icons::render_icon(icon, px(16.0), color).into_any_element()
}

impl LuminaApp {
    pub(crate) fn render_toolbar(
        &self,
        view_label: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_label = view_label.to_string();
        // Only Recently Deleted shows a count and its bulk actions.
        let trash_count = if self.selection == AlbumSelection::Trash {
            self.system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .copied()
                .unwrap_or(0)
        } else {
            0
        };
        div()
            .w_full()
            .h(px(52.0))
            .flex_shrink_0()
            .pl(px(84.0))
            .pr_3()
            .bg(theme::bg_app())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme::text())
            // Sidebar toggle button
            .child(
                icon_button("sidebar-toggle-btn", false)
                    .debug_selector(|| "sidebar-toggle".to_string())
                    .tooltip(theme::tooltip_label("Toggle sidebar"))
                    .child(toolbar_icon(crate::icons::Icon::PanelLeft, false))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sidebar(cx);
                    })),
            )
            // Back is ⌘[ only; the design has no toolbar button for it.
            // The page title (v3: 15px semibold), then the Trash item count.
            // A long album or tag name truncates instead of pushing the
            // buttons off the bar.
            .child(
                div()
                    .ml_2()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap_1p5()
                    .child(
                        div()
                            .debug_selector(|| "toolbar-title".to_string())
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child(if let Some(year) = self.active_year {
                                year.to_string()
                            } else {
                                view_label
                            }),
                    )
                    .when(trash_count > 0, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "trash-item-count".to_string())
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(if trash_count == 1 {
                                    "1 item".to_string()
                                } else {
                                    format!("{trash_count} items")
                                }),
                        )
                    }),
            )
            .child(div().flex_1())
            .child(SearchBarView::render_field(self, cx))
            // Trash bulk actions
            .children((trash_count > 0).then(|| {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        theme::control("header-restore-all-btn")
                            .debug_selector(|| "header-restore-all-btn".to_string())
                            .child("Restore All")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.restore_all_trash(cx);
                            })),
                    )
                    .child(
                        theme::control("header-empty-trash-btn")
                            .debug_selector(|| "header-empty-trash-btn".to_string())
                            .bg(theme::danger_bg())
                            .border_color(theme::danger_border())
                            .text_color(theme::danger_text())
                            .child("Empty Trash…")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.prompt_empty_trash(cx);
                            })),
                    )
            }))
            .child(
                icon_button("sort-toolbar-btn", false)
                    .debug_selector(|| "sort-menu-button".to_string())
                    .tooltip(theme::tooltip_label(format!("Sort: {}", self.sort_label())))
                    .child(toolbar_icon(crate::icons::Icon::Sliders, false))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sort_menu(cx);
                    })),
            )
            .child(
                icon_button("tags-toolbar-btn", false)
                    .debug_selector(|| "tags-toolbar-btn".to_string())
                    .tooltip(theme::tooltip_label("Tags"))
                    .child(toolbar_icon(crate::icons::Icon::Tag, false))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_tag_manager(cx);
                    })),
            )
            .child(
                icon_button("inspector-toggle-btn", self.inspector_open)
                    .debug_selector(|| "inspector-toggle".to_string())
                    .tooltip(theme::tooltip_label("Info (I)"))
                    .child(toolbar_icon(crate::icons::Icon::Info, self.inspector_open))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.toggle_inspector(cx);
                    })),
            )
    }
}

impl LuminaApp {
    /// "Items stay here until…" above a non-empty Recently Deleted.
    pub(crate) fn render_trash_note(&self) -> Option<Div> {
        let count = self
            .system_album_counts
            .get(&lumina_core::SystemAlbum::RecentlyDeleted)
            .copied()
            .unwrap_or(0);
        (self.selection == AlbumSelection::Trash && count > 0).then(|| {
            div()
                .debug_selector(|| "trash-note".to_string())
                .flex_shrink_0()
                .px_4()
                .py(px(6.0))
                .border_b_1()
                .border_color(theme::border_subtle())
                .text_xs()
                .text_color(theme::text_subtle())
                .child(
                    "Items stay here until you empty the trash. \
                     Restore puts them back in the library.",
                )
        })
    }
}
