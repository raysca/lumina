use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{DeleteIntent, LuminaApp};
use gpui::*;
use lumina_core::DeletionTarget;

pub struct DeleteConfirmModal;

impl DeleteConfirmModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let state = match app.delete_confirm_state.as_ref() {
            Some(s) => s,
            None => return div().into_any_element(),
        };

        let title = match state.intent {
            DeleteIntent::EmptyTrash => "Empty Trash",
            DeleteIntent::DeleteSelected => "Permanently Delete Selected Media",
        };

        let count = state.item_count;
        let current_target = state.target;

        div()
            .id("delete-modal-backdrop")
            .debug_selector(|| "delete-modal-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::scrim())
            .flex()
            .justify_center()
            .items_center()
            .p_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_delete_modal(cx);
                }),
            )
            .child(
                div()
                    .id("delete-confirm-dialog")
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .w(px(520.0))
                    .bg(theme::bg_app())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded_xl()
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    // Header Bar
                    .child(
                        div()
                            .w_full()
                            .px_6()
                            .py_4()
                            .bg(theme::bg_pane())
                            .border_b_1()
                            .border_color(theme::border())
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .w(px(32.0))
                                            .h(px(32.0))
                                            .rounded_lg()
                                            .bg(theme::danger_bg())
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .text_base()
                                            .text_color(theme::danger_text())
                                            .child(render_icon(
                                                Icon::TriangleAlert,
                                                px(16.0),
                                                theme::danger_text(),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .text_base()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::text())
                                            .child(title),
                                    ),
                            )
                            .child(
                                div()
                                    .id("close-delete-modal-btn")
                                    .debug_selector(|| "delete-modal-close".to_string())
                                    .tooltip(theme::tooltip_label("Cancel"))
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .hover(|s| s.bg(theme::bg_control()))
                                    .cursor_pointer()
                                    .text_sm()
                                    .text_color(theme::text_subtle())
                                    .child(render_icon(Icon::X, px(14.0), theme::text_subtle()))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.close_delete_modal(cx);
                                    })),
                            ),
                    )
                    // Modal Body
                    .child(
                        div()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            // Warning message banner
                            .child(
                                div()
                                    .p_3p5()
                                    .bg(theme::danger_bg())
                                    .border_1()
                                    .border_color(theme::danger_border())
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(theme::danger_text())
                                    .child(format!(
                                        "Are you sure you want to delete {} item{}?",
                                        count,
                                        if count == 1 { "" } else { "s" }
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text_muted())
                                    .child("SELECT ACTION:"),
                            )
                            // Option 1: Move to macOS Trash / System Recycle Bin (Default)
                            .child(render_option_card(
                                "opt-system-trash",
                                "Move to Trash (Default)",
                                "Safely moves original files to macOS ~/.Trash. Can still be recovered via Finder.",
                                current_target == DeletionTarget::SystemTrash,
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::SystemTrash, cx);
                                }),
                            ))
                            // Option 2: Remove from Lumina library only
                            .child(render_option_card(
                                "opt-catalog-only",
                                "Remove from Lumina",
                                "Removes items from catalog index without touching original files on disk.",
                                current_target == DeletionTarget::CatalogOnly,
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::CatalogOnly, cx);
                                }),
                            ))
                            // Option 3: Delete permanently from disk
                            .child(render_option_card(
                                "opt-disk-permanent",
                                "Delete permanently",
                                "Physically unlinks and deletes files from disk immediately. Frees storage space.",
                                current_target == DeletionTarget::PermanentDisk,
                                true,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::PermanentDisk, cx);
                                }),
                            ))
                            // Acknowledgement checkbox for Permanent Delete
                            .child(if current_target == DeletionTarget::PermanentDisk {
                                div()
                                    .id("delete-permanent-ack-row")
                                    .px_3()
                                    .py_2p5()
                                    .bg(theme::danger_bg())
                                    .border_1()
                                    .border_color(theme::danger_border())
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2p5()
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_delete_permanent_acknowledged(cx);
                                    }))
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(theme::danger())
                                            .bg(if state.permanent_acknowledged {
                                                theme::danger()
                                            } else {
                                                theme::bg_pane()
                                            })
                                            .flex()
                                            .justify_center()
                                            .items_center()
                                            .text_xs()
                                            .text_color(theme::accent_fg())
                                            .child(if state.permanent_acknowledged {
                                                render_icon(
                                                    Icon::Check,
                                                    px(10.0),
                                                    theme::accent_fg(),
                                                )
                                                .into_any_element()
                                            } else {
                                                Empty.into_any_element()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::danger_text())
                                            .child("I understand this permanently removes files and cannot be undone."),
                                    )
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    )
                    // Footer Actions
                    .child({
                        let can_confirm = current_target != DeletionTarget::PermanentDisk || state.permanent_acknowledged;
                        let outcome_label = match current_target {
                            DeletionTarget::SystemTrash => {
                                match state.intent {
                                    DeleteIntent::EmptyTrash => "Empty Trash",
                                    DeleteIntent::DeleteSelected => "Move to Trash",
                                }
                            }
                            DeletionTarget::CatalogOnly => "Remove from Lumina",
                            DeletionTarget::PermanentDisk => "Delete Permanently",
                        };

                        div()
                            .px_6()
                            .py_4()
                            .bg(theme::bg_pane())
                            .border_t_1()
                            .border_color(theme::border())
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .id("cancel-delete-btn")
                                    .px_4()
                                    .py_2()
                                    .bg(theme::bg_control())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child("Cancel (Esc)")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.close_delete_modal(cx);
                                    })),
                            )
                            .child(if can_confirm {
                                let is_destructive = current_target == DeletionTarget::PermanentDisk;
                                div()
                                    .id("confirm-permanent-delete-btn")
                                    .px_5()
                                    .py_2()
                                    .bg(if is_destructive { theme::danger() } else { theme::accent() })
                                    .hover(move |s| {
                                        if is_destructive {
                                            s.bg(theme::danger())
                                        } else {
                                            s.bg(theme::accent_hover())
                                        }
                                    })
                                    .rounded_lg()
                                    .cursor_pointer()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_destructive { theme::text() } else { theme::accent_fg() })
                                    .child(outcome_label)
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.execute_confirmed_deletion(cx);
                                    }))
                                    .into_any_element()
                            } else {
                                div()
                                    .id("confirm-permanent-delete-btn")
                                    .px_5()
                                    .py_2()
                                    .bg(theme::bg_control())
                                    .rounded_lg()
                                    .cursor_not_allowed()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text_disabled())
                                    .child(outcome_label)
                                    .into_any_element()
                            })
                    }),
            )
            .into_any_element()
    }
}

fn render_option_card(
    id_name: &'static str,
    title: &str,
    description: &str,
    is_selected: bool,
    is_destructive: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let active_color = if is_destructive {
        theme::danger()
    } else {
        theme::accent()
    };
    let border_color = if is_selected {
        active_color
    } else {
        theme::border()
    };
    let bg_color = if is_selected {
        if is_destructive {
            theme::danger_bg()
        } else {
            theme::selection_bg()
        }
    } else {
        theme::bg_pane()
    };

    div()
        .id(id_name)
        .p_3()
        .bg(bg_color)
        .border_1()
        .border_color(border_color)
        .rounded_lg()
        .hover(move |s| s.border_color(active_color))
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .on_click(on_click)
        .child(
            // Radio circle indicator
            div()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .border_2()
                .border_color(if is_selected {
                    active_color
                } else {
                    theme::text_disabled()
                })
                .flex()
                .justify_center()
                .items_center()
                .child(if is_selected {
                    div()
                        .w(px(8.0))
                        .h(px(8.0))
                        .rounded_full()
                        .bg(active_color)
                        .into_any_element()
                } else {
                    Empty.into_any_element()
                }),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(if is_selected {
                            theme::text()
                        } else {
                            theme::text_muted()
                        })
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child(description.to_string()),
                ),
        )
}
