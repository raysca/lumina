use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use gpui::*;

pub fn render_trash_lifecycle(weak_app: WeakEntity<LuminaApp>) -> AnyElement {
    let app_restore = weak_app.clone();
    let app_delete = weak_app;

    div()
        .p_3()
        .rounded_lg()
        .bg(theme::danger_bg())
        .border_1()
        .border_color(theme::danger_border())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::danger_text())
                .child("TRASH LIFECYCLE"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .id("inspector-restore-single-btn")
                        .flex_1()
                        .py_1p5()
                        .bg(theme::selection_bg())
                        .hover(|s| s.bg(theme::selection_bg_hover()))
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .justify_center()
                        .items_center()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::accent())
                        .child(render_icon(Icon::Undo2, px(12.0), theme::accent()))
                        .child("Put Back")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_restore.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.trash_or_restore_selected(cx);
                                });
                            }
                        }),
                )
                .child(
                    div()
                        .id("inspector-delete-single-btn")
                        .flex_1()
                        .py_1p5()
                        .bg(theme::danger_bg())
                        .hover(|s| s.bg(theme::danger_bg_hover()))
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .justify_center()
                        .items_center()
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap_1p5()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text())
                        .child(render_icon(Icon::X, px(12.0), theme::text()))
                        .child("Delete Now")
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_delete.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.prompt_delete_selected(cx);
                                });
                            }
                        }),
                ),
        )
        .into_any_element()
}
