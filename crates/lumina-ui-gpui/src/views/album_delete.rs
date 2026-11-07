//! Confirmation before an album is deleted.

use gpui::*;

use crate::i18n::t;
use crate::views::smart_album_delete::confirm_dialog;
use crate::LuminaApp;

/// The dialog for `albums.confirm_delete`, or nothing when it is unset.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(id) = app.albums.confirm_delete else {
        return Empty.into_any_element();
    };
    let heading = match app.album_title(id) {
        Some(title) => t("album.delete_title").replace("{title}", title),
        None => t("album.delete_title_unknown"),
    };
    confirm_dialog(
        "album-delete",
        heading,
        t("album.delete_body"),
        (
            t("album.delete_cancel").into(),
            t("album.delete_confirm").into(),
        ),
        LuminaApp::cancel_delete_album,
        LuminaApp::confirm_delete_album,
        cx,
    )
}
