use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::album_builder::AlbumBuilderModal;
use crate::views::context_menu::MediaContextMenu;
use crate::views::delete_modal::DeleteConfirmModal;

use crate::views::sidebar::AlbumSelection;
use crate::views::tag_manager::TagManagerModal;
use crate::views::viewer::MediaViewer;

impl LuminaApp {
    /// Selection dock: centred over the grid column, 20px above its bottom
    /// (so it never covers the status bar), for any selection.
    pub(crate) fn render_action_dock(
        &self,
        is_fullscreen_viewer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let nothing = self.selected_item.is_none() && self.selected_indices.is_empty();
        if nothing || is_fullscreen_viewer {
            return Empty.into_any_element();
        }
        let in_trash = self.selection == AlbumSelection::Trash;
        let separator = || div().w(px(1.0)).h(px(20.0)).bg(theme::border_strong());

        let mut dock = div()
            .id("multi-select-action-dock")
            .debug_selector(|| "multi-select-action-dock".to_string())
            // Clicks on the dock must not fall through to the grid,
            // which would clear the selection the dock acts on.
            .occlude()
            .p(px(5.0))
            .bg(theme::bg_hud())
            .border_1()
            .border_color(theme::border_strong())
            .rounded(theme::RADIUS_LG)
            .shadow_lg()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .text_xs()
            .child(
                div()
                    .debug_selector(|| "batch-dock-count".to_string())
                    .px(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(format!("{} selected", self.batch_summary.count)),
            )
            .child(separator());

        if in_trash {
            dock = dock
                .child(
                    dock_button("batch-restore-dock-btn", "Restore", None, theme::text()).on_click(
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.restore_selected_trash(cx);
                        }),
                    ),
                )
                .child(separator())
                .child(
                    dock_button(
                        "batch-delete-immediately-dock-btn",
                        "Delete Immediately",
                        None,
                        theme::danger_text(),
                    )
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.prompt_delete_selected(cx);
                    })),
                );
        } else {
            dock = dock
                .child(
                    dock_button("batch-fav-dock-btn", "Favorite", None, theme::text()).on_click(
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            if this.selected_indices.len() > 1 {
                                this.batch_toggle_favorite(cx);
                            } else {
                                this.toggle_favorite_selected(cx);
                            }
                        }),
                    ),
                )
                .child(
                    dock_button("batch-tags-dock-btn", "Tag", None, theme::text())
                        .gap(px(6.0))
                        .child(theme::kbd("T").debug_selector(|| "batch-tags-dock-key".to_string()))
                        .tooltip(theme::tooltip_label("Tag selection (T)"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.begin_tagging_selection(cx);
                        })),
                )
                .child(
                    dock_button("batch-export-dock-btn", "Export…", None, theme::text()).on_click(
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.export_pick_folder(lumina_core::ExportPreset::Originals, cx);
                        }),
                    ),
                )
                .child(separator())
                .child(
                    dock_button(
                        "batch-trash-dock-btn",
                        "",
                        Some(Icon::Trash),
                        theme::danger_text(),
                    )
                    .tooltip(theme::tooltip_label("Delete… (⌫)"))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.prompt_delete_selected(cx);
                    })),
                );
        }

        div()
            .absolute()
            .bottom(px(20.0))
            .left_0()
            .right_0()
            // Centre on the grid, not the grid plus the year rail.
            .when(!self.timeline_year_buckets.is_empty(), |d| {
                d.pr(px(crate::views::grid::YEAR_RAIL_WIDTH))
            })
            .flex()
            .justify_center()
            .child(
                dock.child(
                    dock_button(
                        "batch-clear-dock-btn",
                        "",
                        Some(Icon::X),
                        theme::text_muted(),
                    )
                    .tooltip(theme::tooltip_label("Clear selection (Esc)"))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.clear_selection(cx);
                    })),
                ),
            )
            .into_any_element()
    }

    /// Fullscreen viewer modal overlay
    /// Fades in on every open (a fade, so under Reduce Motion too).
    pub(crate) fn render_viewer_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.is_fullscreen_viewer && self.selected_item.is_some();
        let viewer = if open {
            div()
                .absolute()
                .inset_0()
                // Block hit-testing to the toolbar and grid painted beneath the
                // fullscreen viewer. Children painted after this hitbox (the
                // viewer itself) stay interactive.
                .occlude()
                .child(
                    self.render_slideshow_stage(MediaViewer::render(self, cx).into_any_element()),
                )
                .into_any_element()
        } else {
            Empty.into_any_element()
        };
        crate::motion::overlay_in(open, viewer, "viewer-in")
    }

    /// Smart album builder modal overlay
    pub(crate) fn render_album_builder_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.album_draft.modal_open {
            div()
                .absolute()
                .inset_0()
                .child(AlbumBuilderModal::render(self, cx))
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }

    /// Delete / Empty Trash confirmation modal overlay
    pub(crate) fn render_delete_confirm_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.delete_confirm_state.is_some() {
            div()
                .absolute()
                .inset_0()
                .child(DeleteConfirmModal::render(self, cx))
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }

    /// Media card right-click context menu overlay
    pub(crate) fn render_context_menu_overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.context_menu_state.is_some() {
            MediaContextMenu::render(self, window, cx).into_any_element()
        } else {
            Empty.into_any_element()
        }
    }

    /// Tag Manager modal overlay
    pub(crate) fn render_tag_manager_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.tag_manager.open {
            div()
                .absolute()
                .inset_0()
                .child(TagManagerModal::render(self, cx))
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
    }
}

/// Flat dock button: a text label, or an icon alone when `label` is empty.
fn dock_button(
    id: &'static str,
    label: &'static str,
    icon: Option<Icon>,
    color: Rgba,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .h(px(28.0))
        .min_w(px(28.0))
        .px(px(if label.is_empty() { 0.0 } else { 10.0 }))
        .rounded(theme::RADIUS_MD)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(theme::bg_control_hover()))
        .text_color(color)
        .children(icon.map(|icon| render_icon(icon, px(14.0), color)))
        .when(!label.is_empty(), |d| d.child(label))
}
