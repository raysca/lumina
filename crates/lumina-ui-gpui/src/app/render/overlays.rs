use gpui::*;

use lumina_core::{SortDirection, SortField};

use crate::app::types::LayoutMode;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::album_builder::AlbumBuilderModal;
use crate::views::context_menu::MediaContextMenu;
use crate::views::delete_modal::DeleteConfirmModal;
use crate::views::inspector::format_bytes;
use crate::views::sidebar::AlbumSelection;
use crate::views::tag_manager::TagManagerModal;
use crate::views::viewer::MediaViewer;

impl LuminaApp {
    /// Floating Multi-Select Action Dock (Bottom Center)
    pub(crate) fn render_action_dock(
        &self,
        is_fullscreen_viewer: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.selected_indices.len() <= 1 || is_fullscreen_viewer {
            return Empty.into_any_element();
        }

        let count = self.batch_summary.count;
        let total_size_str = format_bytes(self.batch_summary.total_bytes);

        div()
            .absolute()
            .bottom_6()
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .items_center()
            .child(
                div()
                    .id("multi-select-action-dock")
                    .px_5()
                    .py_2p5()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::accent())
                    .rounded_xl()
                    .shadow_lg()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(render_icon(Icon::Sparkles, px(14.0), theme::accent()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text())
                                    .child(format!(
                                        "{} items selected ({})",
                                        count, total_size_str
                                    )),
                            ),
                    )
                    // Action: Batch Favorite (hidden in Trash view)
                    .child(if self.selection != AlbumSelection::Trash {
                        div()
                            .id("batch-fav-dock-btn")
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .text_color(theme::rating())
                            .child(render_icon(Icon::HeartFilled, px(12.0), theme::rating()))
                            .child("Favorite")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.batch_toggle_favorite(cx);
                            }))
                            .into_any_element()
                    } else {
                        Empty.into_any_element()
                    })
                    // Action: Batch Trash or Restore / Delete Immediately
                    .child(if self.selection == AlbumSelection::Trash {
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .id("batch-restore-dock-btn")
                                    .px_2p5()
                                    .py_1()
                                    .bg(theme::selection_bg())
                                    .hover(|s| s.bg(theme::selection_bg_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .text_color(theme::accent())
                                    .child(render_icon(Icon::Undo2, px(12.0), theme::accent()))
                                    .child("Restore")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.restore_selected_trash(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("batch-delete-immediately-dock-btn")
                                    .px_2p5()
                                    .py_1()
                                    .bg(theme::danger_bg())
                                    .hover(|s| s.bg(theme::danger_bg_hover()))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child(render_icon(Icon::X, px(12.0), theme::text()))
                                    .child("Delete Immediately")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.prompt_delete_selected(cx);
                                    })),
                            )
                            .into_any_element()
                    } else {
                        div()
                            .id("batch-trash-dock-btn")
                            .px_2p5()
                            .py_1()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::danger_bg()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .text_color(theme::danger_text())
                            .child(render_icon(Icon::Trash, px(12.0), theme::danger_text()))
                            .child("Trash")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.batch_trash_or_restore(cx);
                            }))
                            .into_any_element()
                    })
                    // Action: Open Tags Editor / Inspector
                    .child(
                        div()
                            .id("batch-tags-dock-btn")
                            .px_2p5()
                            .py_1()
                            .bg(theme::accent())
                            .hover(|s| s.bg(theme::accent_hover()).text_color(theme::accent_fg()))
                            .rounded_md()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme::accent_fg())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(render_icon(Icon::Tag, px(12.0), theme::accent_fg()))
                            .child("Add Tags")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.inspector_open = true;
                                cx.notify();
                            })),
                    )
                    // Action: Export Selection
                    .child(
                        div()
                            .id("batch-export-dock-btn")
                            .px_2()
                            .py_1()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(render_icon(Icon::Folder, px(12.0), theme::text_muted()))
                            .child("Export…")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.export_pick_folder(cx);
                            })),
                    )
                    // Action: Clear Selection
                    .child(
                        div()
                            .id("batch-clear-dock-btn")
                            .px_2()
                            .py_1()
                            .bg(theme::bg_control())
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(render_icon(Icon::X, px(12.0), theme::text_muted()))
                            .child("Clear (Esc)")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.clear_selection(cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// Fullscreen viewer modal overlay
    pub(crate) fn render_viewer_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_fullscreen_viewer
            && self.layout_mode != LayoutMode::Viewer
            && self.selected_item.is_some()
        {
            div()
                .absolute()
                .inset_0()
                .child(MediaViewer::render(self, cx))
                .into_any_element()
        } else {
            Empty.into_any_element()
        }
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

    /// Sort field / direction popover (toolbar).
    pub(crate) fn render_sort_menu_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.sort_menu_open {
            return Empty.into_any_element();
        }
        let current = self.sort;
        let fields: [(&str, SortField, &'static str); 4] = [
            ("Date taken", SortField::DateTaken, "sort-field-date"),
            ("Name", SortField::FileName, "sort-field-name"),
            ("File size", SortField::FileSize, "sort-field-size"),
            ("Rating", SortField::Rating, "sort-field-rating"),
        ];
        div()
            .absolute()
            .inset_0()
            .child(div().absolute().inset_0().on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_sort_menu(cx);
                }),
            ))
            .child(
                div()
                    .absolute()
                    .top(px(58.0))
                    .right(px(16.0))
                    .w(px(210.0))
                    .p_2()
                    .bg(theme::bg_pane())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_lg()
                    .shadow_2xl()
                    .occlude()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text_subtle())
                            .child("SORT BY"),
                    )
                    .children(fields.into_iter().map(|(label, field, selector)| {
                        let selected = current.field == field;
                        div()
                            .id(selector)
                            .debug_selector(move || selector.to_string())
                            .px_2()
                            .py_1p5()
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .text_xs()
                            .text_color(if selected {
                                theme::accent()
                            } else {
                                theme::text()
                            })
                            .hover(|s| s.bg(theme::bg_control_hover()))
                            .child(label.to_string())
                            .child(if selected { "✓" } else { "" })
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.set_sort_field(field, cx);
                            }))
                    }))
                    .child(div().h(px(1.0)).bg(theme::border_subtle()).my_1())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .px_1()
                            .child(
                                div()
                                    .id("sort-direction-asc")
                                    .debug_selector(|| "sort-direction-asc".to_string())
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_center()
                                    .text_color(if current.direction == SortDirection::Ascending {
                                        theme::accent()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .bg(if current.direction == SortDirection::Ascending {
                                        theme::selection_bg()
                                    } else {
                                        theme::bg_control()
                                    })
                                    .child("Ascending ↑")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.set_sort_direction(SortDirection::Ascending, cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("sort-direction-desc")
                                    .debug_selector(|| "sort-direction-desc".to_string())
                                    .flex_1()
                                    .px_2()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_center()
                                    .text_color(if current.direction == SortDirection::Descending {
                                        theme::accent()
                                    } else {
                                        theme::text_muted()
                                    })
                                    .bg(if current.direction == SortDirection::Descending {
                                        theme::selection_bg()
                                    } else {
                                        theme::bg_control()
                                    })
                                    .child("Descending ↓")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.set_sort_direction(SortDirection::Descending, cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
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
