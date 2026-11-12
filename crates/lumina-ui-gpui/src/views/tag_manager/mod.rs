//! The Tags page (frame `L3F8Cs`): the keyword list on the left and the
//! focused keyword's details on the right, in the main column under the page
//! header while `AlbumSelection::Tags` shows. State lives in `app/tags.rs`,
//! keys in `app/keymap/tags_page.rs`.

pub mod detail;
pub mod list;

use gpui::*;

use crate::theme;
use crate::LuminaApp;

/// The page body under the header.
pub(crate) fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let rows = app.tag_manager_rows();
    div()
        .id("tags-page")
        .debug_selector(|| "tags-page".to_string())
        .size_full()
        .bg(theme::bg_0())
        .flex()
        .flex_row()
        .child(list::render_list(app, &rows, cx))
        .child(detail::render_detail(app, cx))
        .into_any_element()
}
