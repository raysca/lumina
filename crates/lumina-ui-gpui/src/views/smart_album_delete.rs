//! Confirmation before a smart album is deleted.

use gpui::*;

use crate::theme;
use crate::LuminaApp;

/// The dialog for `album_draft.confirm_delete`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(album_id) = app.album_draft.confirm_delete else {
        return Empty.into_any_element();
    };
    let title = app
        .user_smart_albums
        .iter()
        .find(|album| album.id == album_id)
        .map_or_else(
            || "this smart album".to_string(),
            |a| format!("“{}”", a.title),
        );

    let button = |id: &'static str, label: &'static str| {
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .h(px(30.0))
            .px_3()
            .rounded(theme::RADIUS_MD)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .child(label)
    };

    div()
        .id("smart-album-delete-backdrop")
        .debug_selector(|| "smart-album-delete-backdrop".to_string())
        .absolute()
        .inset_0()
        .occlude()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_center()
        .p_8()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.cancel_delete_smart_album(cx);
            }),
        )
        .child(
            div()
                .id("smart-album-delete-dialog")
                .debug_selector(|| "smart-album-delete-dialog".to_string())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .w(px(400.0))
                .p_5()
                .bg(theme::bg_dialog())
                .border_1()
                .border_color(theme::border_strong())
                .rounded(theme::RADIUS_LG)
                .shadow_2xl()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_base()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .truncate()
                                .child(format!("Delete {title}?")),
                        )
                        .child(div().text_xs().text_color(theme::text_muted()).child(
                            "Only the album goes. Its photos and videos stay in your library.",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("smart-album-delete-cancel", "Cancel")
                                .bg(theme::bg_control())
                                .hover(|s| s.bg(theme::bg_control_hover()))
                                .text_color(theme::text())
                                .child(theme::kbd("Esc"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.cancel_delete_smart_album(cx);
                                })),
                        )
                        .child(
                            button("smart-album-delete-confirm", "Delete")
                                .bg(theme::danger())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme::text())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.confirm_delete_smart_album(cx);
                                })),
                        ),
                ),
        )
        .into_any_element()
}
