use crate::app::types::format_bytes;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{DeleteIntent, LuminaApp};
use gpui::*;
use lumina_core::DeletionTarget;
use std::sync::Arc;

/// Header thumbnail edge and how far each one overlaps the previous.
const THUMB: f32 = 52.0;
const THUMB_STEP: f32 = 18.0;

pub struct DeleteConfirmModal;

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "2 photos, 1 video · 41.2 MB"; empty for Empty Trash (no per-item data).
fn summary_line(photos: usize, videos: usize, bytes: u64) -> String {
    let mut kinds = Vec::new();
    if photos > 0 {
        kinds.push(plural(photos, "photo", "photos"));
    }
    if videos > 0 {
        kinds.push(plural(videos, "video", "videos"));
    }
    if kinds.is_empty() {
        return String::new();
    }
    format!("{} · {}", kinds.join(", "), format_bytes(bytes))
}

impl DeleteConfirmModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        let state = match app.delete_confirm_state.as_ref() {
            Some(s) => s,
            None => return div().into_any_element(),
        };

        let count = state.item_count;
        let target = state.target;
        let items = plural(count, "item", "items");
        let title = match state.intent {
            DeleteIntent::EmptyTrash => format!("Empty Trash ({items})?"),
            _ => format!("Delete {items}?"),
        };
        let from_library = matches!(
            state.intent,
            DeleteIntent::LibrarySelection | DeleteIntent::Rejected
        );
        let trash_description = if from_library {
            "Keeps them in Recently Deleted. You can restore them at any time."
        } else {
            crate::i18n::platform::MOVE_TO_TRASH_DESCRIPTION
        };
        let thumbs: Vec<Arc<RenderImage>> = app
            .grid_state
            .as_ref()
            .map(|grid| {
                let cache = grid.texture_cache.lock();
                state
                    .preview_ids
                    .iter()
                    .filter_map(|id| cache.peek(id).cloned())
                    .collect()
            })
            .unwrap_or_default();
        let can_confirm = target != DeletionTarget::PermanentDisk || state.permanent_acknowledged;
        let confirm_label = match (target, state.intent) {
            (DeletionTarget::SystemTrash, DeleteIntent::EmptyTrash) => "Empty Trash",
            (DeletionTarget::SystemTrash, _) => "Move to Trash",
            (DeletionTarget::CatalogOnly, _) => "Remove from Library",
            (DeletionTarget::PermanentDisk, _) => "Delete Permanently",
        };
        let destructive = target == DeletionTarget::PermanentDisk;

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_4()
            .children((!thumbs.is_empty()).then(|| {
                let n = thumbs.len() as f32;
                div()
                    .id("delete-thumbs")
                    .debug_selector(|| "delete-thumbs".to_string())
                    .relative()
                    .flex_none()
                    .w(px(THUMB + THUMB_STEP * (n - 1.0)))
                    .h(px(THUMB))
                    .children(thumbs.into_iter().enumerate().map(|(i, image)| {
                        div()
                            .absolute()
                            .left(px(THUMB_STEP * i as f32))
                            .size(px(THUMB))
                            .overflow_hidden()
                            .border_2()
                            .border_color(theme::bg_1())
                            .bg(theme::bg_media())
                            .child(img(image).size_full().object_fit(ObjectFit::Cover))
                    }))
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .id("delete-title")
                            .debug_selector(|| "delete-title".to_string())
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .child(title),
                    )
                    .child(
                        div()
                            .id("delete-summary")
                            .debug_selector(|| "delete-summary".to_string())
                            .text_xs()
                            .text_color(theme::t2())
                            .child(summary_line(state.photos, state.videos, state.total_bytes)),
                    ),
            );

        let ack = (target == DeletionTarget::PermanentDisk).then(|| {
            let on = state.permanent_acknowledged;
            div()
                .id("delete-permanent-ack-row")
                .debug_selector(|| "delete-permanent-ack-row".to_string())
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
                        .size(px(16.0))
                        .flex_none()
                        .rounded(theme::RADIUS_SM)
                        .border_1()
                        .border_color(theme::danger())
                        .bg(if on { theme::danger() } else { theme::bg_0() })
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(on.then(|| render_icon(Icon::Check, px(10.0), theme::bg_0()))),
                )
                .child(div().text_xs().text_color(theme::danger()).child(format!(
                    "I understand {} will be deleted from disk and can't be recovered. (Space)",
                    items
                )))
        });

        let footer = div()
            .flex()
            .flex_row()
            .justify_end()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("cancel-delete-btn")
                    .debug_selector(|| "cancel-delete-btn".to_string())
                    .h(px(30.0))
                    .px_3()
                    .rounded(theme::RADIUS_MD)
                    .bg(theme::bg_2())
                    .hover(|s| s.bg(theme::bg_3()))
                    .cursor_pointer()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(theme::t1())
                    .child("Cancel")
                    .child(theme::kbd("Esc"))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.close_delete_modal(cx);
                    })),
            )
            .child(
                div()
                    .id("confirm-permanent-delete-btn")
                    .debug_selector(|| "confirm-delete-btn".to_string())
                    .h(px(30.0))
                    .px_4()
                    .rounded(theme::RADIUS_MD)
                    .flex()
                    .items_center()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .bg(if destructive {
                        theme::danger()
                    } else {
                        theme::accent()
                    })
                    .text_color(if destructive {
                        theme::bg_0()
                    } else {
                        theme::on_accent()
                    })
                    // Disabled = the same red button at 40%.
                    .opacity(if can_confirm { 1.0 } else { 0.4 })
                    .cursor(if can_confirm {
                        CursorStyle::PointingHand
                    } else {
                        CursorStyle::OperationNotAllowed
                    })
                    .child(confirm_label)
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.execute_confirmed_deletion(cx);
                    })),
            );

        div()
            .id("delete-modal-backdrop")
            .debug_selector(|| "delete-modal-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::modal_dim())
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_3()
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
                    .debug_selector(|| "delete-confirm-dialog".to_string())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .w(px(480.0))
                    .p_5()
                    .bg(theme::bg_1())
                    .border_1()
                    .border_color(theme::line_2())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(header)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(render_option_card(
                                "opt-system-trash",
                                "Move to Trash",
                                trash_description,
                                true,
                                target == DeletionTarget::SystemTrash,
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::SystemTrash, cx);
                                }),
                            ))
                            .child(render_option_card(
                                "opt-catalog-only",
                                "Remove from Lumina only",
                                "Takes them out of your library. The files stay on disk.",
                                false,
                                target == DeletionTarget::CatalogOnly,
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::CatalogOnly, cx);
                                }),
                            ))
                            .child(render_option_card(
                                "opt-disk-permanent",
                                "Delete permanently",
                                "Erases the files from disk right away. This can't be undone.",
                                false,
                                target == DeletionTarget::PermanentDisk,
                                true,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.set_delete_target(DeletionTarget::PermanentDisk, cx);
                                }),
                            )),
                    )
                    .children(ack)
                    .child(footer),
            )
            .children(from_library.then(|| {
                div()
                    .id("delete-hint")
                    .debug_selector(|| "delete-hint".to_string())
                    .text_xs()
                    .text_color(theme::t2())
                    .child(
                        "Trashed items stay in Library › Recently Deleted. ↑↓ choose · ↵ confirm",
                    )
            }))
            .into_any_element()
    }
}

fn render_option_card(
    id_name: &'static str,
    title: &'static str,
    description: &'static str,
    recommended: bool,
    is_selected: bool,
    is_destructive: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let active_color = if is_destructive {
        theme::danger()
    } else {
        theme::accent()
    };

    div()
        .id(id_name)
        .debug_selector(move || id_name.to_string())
        .p_3()
        .bg(if is_selected {
            if is_destructive {
                theme::danger_soft()
            } else {
                theme::bg_3()
            }
        } else {
            theme::bg_2()
        })
        .border_1()
        .border_color(if is_selected {
            active_color
        } else {
            theme::line()
        })
        .rounded(theme::RADIUS_LG)
        .hover(move |s| s.border_color(active_color))
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .on_click(on_click)
        .child(
            div()
                .size(px(16.0))
                .flex_none()
                .rounded_full()
                .border_2()
                .border_color(if is_selected {
                    active_color
                } else {
                    theme::t3()
                })
                .flex()
                .justify_center()
                .items_center()
                .children(is_selected.then(|| div().size(px(8.0)).rounded_full().bg(active_color))),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::SEMIBOLD)
                                // The permanent option's title is always red.
                                .text_color(if is_destructive {
                                    theme::label_red()
                                } else {
                                    theme::t1()
                                })
                                .child(title),
                        )
                        .children(recommended.then(|| {
                            div()
                                .px_1p5()
                                .rounded(theme::RADIUS_SM)
                                .bg(theme::bg_3())
                                .text_size(px(11.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::accent())
                                .child("Recommended")
                        })),
                )
                .child(div().text_xs().text_color(theme::t2()).child(description)),
        )
}

#[cfg(test)]
mod tests {
    use super::summary_line;

    #[core::prelude::v1::test]
    fn summary_counts_kinds_and_size() {
        assert_eq!(summary_line(2, 1, 0), "2 photos, 1 video · 0 B");
        assert_eq!(summary_line(1, 0, 0), "1 photo · 0 B");
        assert_eq!(summary_line(0, 0, 10), "", "Empty Trash has no summary");
    }
}
