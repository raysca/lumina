mod albums;
pub(crate) mod dormant;
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
    /// The Albums overview page: album and smart album cards, no media grid.
    AlbumsOverview,
    /// The Tags page (TAGS › Keywords): the keyword list and detail, no grid.
    Tags,
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
            .gap(px(18.0))
            .py(px(16.0))
            .px(px(12.0))
            .text_color(theme::t1())
            .child(render_brand())
            .child(crate::views::search_bar::SearchBarView::render_field(
                this, cx,
            ))
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .flex_col()
                    .gap(px(18.0))
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

/// Brand row: the aperture mark and "Lumina". On macOS it starts after the
/// traffic lights, which the window draws over the sidebar's top-left.
fn render_brand() -> impl IntoElement {
    // 84px from the window edge, less the sidebar's 12px padding.
    let inset = if cfg!(target_os = "macos") { 72.0 } else { 2.0 };
    div()
        .debug_selector(|| "sidebar-brand".to_string())
        .flex_none()
        // At the 180px minimum the wordmark clips rather than spilling out.
        .min_w_0()
        .overflow_hidden()
        .pl(px(inset))
        .pr(px(2.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(9.0))
        .child(div().flex_none().child(crate::icons::render_icon(
            crate::icons::Icon::Aperture,
            px(19.0),
            theme::accent(),
        )))
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(18.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Lumina"),
        )
}
