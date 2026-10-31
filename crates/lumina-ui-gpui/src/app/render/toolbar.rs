use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::search_bar::SearchBarView;
use crate::views::sidebar::AlbumSelection;

/// 28×28 icon-only toolbar button; the tooltip names the action.
fn icon_button(id: &'static str, on: bool) -> Stateful<Div> {
    if on {
        theme::control_on(id)
    } else {
        theme::control(id)
    }
    .w(px(28.0))
    .px_0()
    .justify_center()
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
                if self.sidebar_collapsed {
                    theme::control("sidebar-toggle-btn")
                        .debug_selector(|| "sidebar-toggle".to_string())
                        .tooltip(theme::tooltip_label("Toggle sidebar"))
                } else {
                    theme::control_on("sidebar-toggle-btn")
                        .debug_selector(|| "sidebar-toggle".to_string())
                        .tooltip(theme::tooltip_label("Toggle sidebar"))
                }
                .w(px(28.0))
                .px_0()
                .justify_center()
                .child(crate::icons::render_icon(
                    crate::icons::Icon::PanelLeft,
                    px(14.0),
                    theme::text(),
                ))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_sidebar(cx);
                })),
            )
            // Back is ⌘[ only; the design has no toolbar button for it.
            // Breadcrumb: Library / year-view
            .child(
                div()
                    .ml_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .text_xs()
                    .child(
                        div()
                            .id("breadcrumb-library-btn")
                            .debug_selector(|| "breadcrumb-library".to_string())
                            .cursor_pointer()
                            .hover(|s| s.text_color(theme::text()))
                            .text_color(theme::text_muted())
                            .child("Library")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.select_album(AlbumSelection::AllLibrary, cx);
                            })),
                    )
                    .child(div().text_color(theme::text_disabled()).child(
                        crate::icons::render_icon(
                            crate::icons::Icon::ChevronRight,
                            px(10.0),
                            theme::text_disabled(),
                        ),
                    ))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
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
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Sliders,
                        px(14.0),
                        theme::text_muted(),
                    ))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sort_menu(cx);
                    })),
            )
            .child(
                icon_button("tags-toolbar-btn", false)
                    .debug_selector(|| "tags-toolbar-btn".to_string())
                    .tooltip(theme::tooltip_label("Tags"))
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Tag,
                        px(14.0),
                        theme::text_muted(),
                    ))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_tag_manager(cx);
                    })),
            )
            .child(
                icon_button("inspector-toggle-btn", self.inspector_open)
                    .debug_selector(|| "inspector-toggle".to_string())
                    .tooltip(theme::tooltip_label("Info (I)"))
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Info,
                        px(14.0),
                        if self.inspector_open {
                            theme::selection_fg()
                        } else {
                            theme::text_muted()
                        },
                    ))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.toggle_inspector(cx);
                    })),
            )
            // ponytail: the design's primary is Import; Lumina has no import
            // yet, so the amber slot switches the library folder.
            .child(
                theme::control_primary("change-folder-btn")
                    .debug_selector(|| "switch-library".to_string())
                    .tooltip(theme::tooltip_label("Switch Library…"))
                    .w(px(28.0))
                    .px_0()
                    .justify_center()
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Folder,
                        px(14.0),
                        theme::accent_fg(),
                    ))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.pick_folder_dialog(cx);
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
