mod albums;
pub(crate) mod nav;
mod rail;
mod sections;

use gpui::*;
use lumina_core::SystemAlbum;

use crate::theme;
use crate::LuminaApp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbumSelection {
    AllLibrary,
    System(SystemAlbum),
    User(i64),
    /// A hand-picked album (its own id space, not a smart album).
    Album(i64),
    Tag(String),
    /// Active media that carries no tags at all.
    Untagged,
    Duplicates,
    Trash,
}

pub fn tag_color(name: &str) -> Rgba {
    match name.to_lowercase().as_str() {
        "family" => theme::label_orange(),
        "travel" => theme::label_yellow(),
        "street" => theme::label_blue(),
        "work" => theme::label_purple(),
        "portrait" => theme::label_red(),
        "nature" => theme::label_green(),
        _ => {
            let hash = name
                .bytes()
                .fold(0usize, |acc, b| acc.wrapping_add(b as usize));
            let palette = [
                theme::label_orange(),
                theme::label_yellow(),
                theme::label_blue(),
                theme::label_green(),
                theme::label_purple(),
                theme::label_red(),
            ];
            palette[hash % palette.len()]
        }
    }
}

pub struct SidebarView;

impl SidebarView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        if this.sidebar_collapsed {
            return rail::render_rail(this, cx).into_any_element();
        }

        div()
            .debug_selector(|| "sidebar".to_string())
            .w(px(this.sidebar_width))
            .h_full()
            .bg(theme::bg_1())
            .border_r_1()
            .border_color(theme::line())
            .flex()
            .flex_col()
            .justify_between()
            .py(px(4.0))
            .px(px(8.0))
            .text_color(theme::t1())
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex()
                    .flex_1()
                    .flex_col()
                    .gap_0()
                    .overflow_y_scroll()
                    .child(sections::render_library_section(this, cx))
                    .child(albums::render_pinned_section(this, cx))
                    .child(albums::render_albums_section(this, cx))
                    .child(sections::render_smart_albums_section(this, cx))
                    .child(sections::render_tags_section(this, cx)),
            )
            .into_any_element()
    }
}
