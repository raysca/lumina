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
            return div()
                .w(px(48.0))
                .h_full()
                .bg(theme::bg_app())
                .border_r_1()
                .border_color(theme::border())
                .flex()
                .flex_col()
                .items_center()
                .py_4()
                .gap_4()
                .child(
                    div()
                        .id("expand-sidebar-btn")
                        .debug_selector(|| "expand-sidebar".to_string())
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded(theme::RADIUS_MD)
                        .bg(theme::bg_raised())
                        .hover(|s| s.bg(theme::bg_control()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .text_color(theme::text_muted())
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::PanelLeft,
                            px(16.0),
                            theme::text_muted(),
                        ))
                        .tooltip(theme::tooltip_label("Expand sidebar"))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_sidebar(cx);
                        })),
                )
                .into_any_element();
        }

        div()
            .debug_selector(|| "sidebar".to_string())
            .w(px(this.sidebar_width))
            .h_full()
            .bg(theme::bg_pane())
            .border_r_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_col()
            .justify_between()
            .py(px(4.0))
            .px(px(8.0))
            .text_color(theme::text())
            .child(
                div()
                    .id("sidebar-scroll")
                    .flex()
                    .flex_1()
                    .flex_col()
                    .gap_0()
                    .overflow_y_scroll()
                    .child(sections::render_library_section(this, cx))
                    .child(sections::render_smart_albums_section(this, cx))
                    .child(sections::render_tags_section(this, cx)),
            )
            .into_any_element()
    }
}
