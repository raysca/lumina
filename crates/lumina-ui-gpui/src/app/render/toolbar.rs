use gpui::*;

use crate::app::types::LayoutMode;
use crate::app::LuminaApp;
use crate::theme;
use crate::views::search_bar::SearchBarView;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    pub(crate) fn render_toolbar(
        &self,
        view_label: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_label = view_label.to_string();
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
                        .tooltip(theme::tooltip_label("Toggle sidebar"))
                } else {
                    theme::control_on("sidebar-toggle-btn")
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
            // Segmented mode switcher: [ Browser | Split | Viewer ]
            .child(
                theme::segmented("layout-mode-switcher")
                    .child(
                        theme::segmented_item(
                            "mode-browser",
                            self.layout_mode == LayoutMode::Browser,
                        )
                        .child("Browser")
                        .on_click(cx.listener(
                            |this, _e: &ClickEvent, _w, cx| {
                                this.layout_mode = LayoutMode::Browser;
                                this.is_fullscreen_viewer = false;
                                this.save_config();
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        theme::segmented_item("mode-split", self.layout_mode == LayoutMode::Split)
                            .child("Split")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.layout_mode = LayoutMode::Split;
                                this.save_config();
                                cx.notify();
                            })),
                    )
                    .child(
                        theme::segmented_item(
                            "mode-viewer",
                            self.layout_mode == LayoutMode::Viewer,
                        )
                        .child("Viewer")
                        .on_click(cx.listener(
                            |this, _e: &ClickEvent, _w, cx| {
                                this.layout_mode = LayoutMode::Viewer;
                                this.is_fullscreen_viewer = true;
                                this.save_config();
                                cx.notify();
                            },
                        )),
                    ),
            )
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
                    ),
            )
            .child(div().flex_1())
            .child(SearchBarView::render_field(self, cx))
            // Trash bulk actions
            .children((self.selection == AlbumSelection::Trash).then(|| {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        theme::control("header-restore-all-btn")
                            .child("Restore All")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.restore_all_trash(cx);
                            })),
                    )
                    .child(
                        theme::control("header-empty-trash-btn")
                            .bg(theme::danger_bg())
                            .border_color(theme::danger_border())
                            .text_color(theme::danger_text())
                            .hover(|s| s.bg(theme::danger_bg_hover()))
                            .child("Empty Trash")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.prompt_empty_trash(cx);
                            })),
                    )
            }))
            .child(
                theme::control("tags-toolbar-btn")
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Tag,
                        px(13.0),
                        theme::text_muted(),
                    ))
                    .child("Tags")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_tag_manager(cx);
                    })),
            )
            .child(
                if self.inspector_open {
                    theme::control_on("inspector-toggle-btn")
                } else {
                    theme::control("inspector-toggle-btn")
                }
                .child(crate::icons::render_icon(
                    crate::icons::Icon::Info,
                    px(13.0),
                    if self.inspector_open {
                        theme::selection_fg()
                    } else {
                        theme::text_muted()
                    },
                ))
                .child("Info")
                .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.toggle_inspector(cx);
                })),
            )
            .child(
                theme::control_primary("change-folder-btn")
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(crate::icons::render_icon(
                        crate::icons::Icon::Folder,
                        px(13.0),
                        theme::accent_fg(),
                    ))
                    .child("Switch Library…")
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.pick_folder_dialog(cx);
                    })),
            )
    }
}
