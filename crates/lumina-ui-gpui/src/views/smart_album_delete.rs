//! Confirmation before a smart album (or an album) is deleted.

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
    confirm_dialog(
        "smart-album-delete",
        format!("Delete {title}?"),
        "Only the album goes. Its photos and videos stay in your library.".into(),
        ("Cancel".into(), "Delete".into()),
        LuminaApp::cancel_delete_smart_album,
        LuminaApp::confirm_delete_smart_album,
        cx,
    )
}

type Handler = fn(&mut LuminaApp, &mut Context<LuminaApp>);

/// A delete confirmation card over a dimmed backdrop. Selectors are
/// `{prefix}-backdrop`, `-dialog`, `-cancel` and `-confirm`; a click on the
/// backdrop cancels. `buttons` are the (cancel, confirm) labels.
pub(crate) fn confirm_dialog(
    prefix: &'static str,
    heading: String,
    body: String,
    buttons: (SharedString, SharedString),
    cancel: Handler,
    confirm: Handler,
    cx: &mut Context<LuminaApp>,
) -> AnyElement {
    let name = move |part: &str| format!("{prefix}-{part}");
    let button = |part: &str, label: SharedString| {
        let selector = name(part);
        div()
            .id(ElementId::Name(selector.clone().into()))
            .debug_selector(move || selector.clone())
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
    let (backdrop, dialog) = (name("backdrop"), name("dialog"));

    div()
        .id(ElementId::Name(backdrop.clone().into()))
        .debug_selector(move || backdrop.clone())
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
            cx.listener(move |this, _e: &MouseDownEvent, _w, cx| cancel(this, cx)),
        )
        .child(
            div()
                .id(ElementId::Name(dialog.clone().into()))
                .debug_selector(move || dialog.clone())
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                    cx.stop_propagation();
                })
                .w(px(400.0))
                .p_5()
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::line_2())
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
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::t1())
                                .truncate()
                                .child(heading),
                        )
                        .child(div().text_xs().text_color(theme::t2()).child(body)),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .child(
                            button("cancel", buttons.0)
                                .bg(theme::bg_2())
                                .hover(|s| s.bg(theme::bg_3()))
                                .text_color(theme::t1())
                                .child(theme::kbd("Esc"))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    cancel(this, cx)
                                })),
                        )
                        .child(
                            button("confirm", buttons.1)
                                .bg(theme::danger())
                                .hover(|s| s.opacity(0.9))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme::bg_0())
                                .child(theme::kbd("↵"))
                                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                    confirm(this, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
}
