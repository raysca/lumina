//! "Remove this library?" confirmation for
//! [`crate::app::library_delete`].

use crate::theme;
use crate::LuminaApp;
use gpui::*;

impl LuminaApp {
    /// The library-delete card, or nothing when no confirmation is pending.
    pub fn render_library_delete_modal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(state) = self.library_delete.as_ref() else {
            return div().into_any_element();
        };
        let name = crate::app::recents::library_name(&state.path);
        let path = state.path.display().to_string();
        let title = crate::i18n::t("library.delete_title").replace("{name}", &name);
        let body = crate::i18n::t("library.delete_body").replace("{path}", &path);

        div()
            .id("library-delete-backdrop")
            .debug_selector(|| "library-delete-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::modal_dim())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_8()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.cancel_delete_library(cx);
                }),
            )
            .child(
                div()
                    .id("library-delete-dialog")
                    .debug_selector(|| "library-delete-dialog".to_string())
                    .role(Role::AlertDialog)
                    .aria_label(title.clone())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                    .w(px(460.0))
                    .p_5()
                    .bg(theme::bg_1())
                    .border_1()
                    .border_color(theme::line_2())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .child(title),
                    )
                    .child(div().text_xs().text_color(theme::t2()).child(body))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap_2()
                            .child(
                                div()
                                    .id("library-delete-cancel")
                                    .debug_selector(|| "library-delete-cancel".to_string())
                                    .role(Role::Button)
                                    .aria_label(crate::i18n::t("library.delete_cancel"))
                                    .h(px(30.0))
                                    .px_4()
                                    .rounded(theme::RADIUS_MD)
                                    .flex()
                                    .items_center()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(theme::bg_2())
                                    .text_color(theme::t1())
                                    .cursor_pointer()
                                    .child(crate::i18n::t("library.delete_cancel"))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.cancel_delete_library(cx);
                                    })),
                            )
                            .child(
                                div()
                                    .id("library-delete-confirm")
                                    .debug_selector(|| "library-delete-confirm".to_string())
                                    .role(Role::Button)
                                    .aria_label(crate::i18n::t("library.delete_confirm"))
                                    .h(px(30.0))
                                    .px_4()
                                    .rounded(theme::RADIUS_MD)
                                    .flex()
                                    .items_center()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .bg(theme::danger())
                                    .text_color(theme::bg_0())
                                    .cursor_pointer()
                                    .child(crate::i18n::t("library.delete_confirm"))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.confirm_delete_library(cx);
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}

/// ⌘K "Delete Library…": asks about the open library, or the recent the
/// welcome screen's keyboard has focused.
pub(crate) fn prompt_open_library_delete(app: &mut LuminaApp, cx: &mut Context<LuminaApp>) {
    let path = app
        .grid_state
        .as_ref()
        .map(|grid| grid.engine.library_root().to_path_buf())
        .or_else(|| {
            app.welcome
                .focused_recent
                .and_then(|index| app.config.recent_libraries.get(index).cloned())
        });
    if let Some(path) = path {
        app.prompt_delete_library(path, cx);
    }
}
