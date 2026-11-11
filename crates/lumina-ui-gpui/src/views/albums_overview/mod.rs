//! The Albums overview page (frame `a0sLrG`): the main column's body while
//! `AlbumSelection::AlbumsOverview` shows.

use gpui::*;

use crate::LuminaApp;

/// The page body. Album and smart album cards fill it in later tasks.
pub(crate) fn render(_this: &LuminaApp, _cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .id("albums-overview")
        .debug_selector(|| "albums-overview".to_string())
        .size_full()
        .overflow_y_scroll()
        .into_any_element()
}
