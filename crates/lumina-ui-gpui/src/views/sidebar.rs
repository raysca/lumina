use gpui::*;
use lumina_core::SystemAlbum;
use crate::LuminaApp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlbumSelection {
    AllLibrary,
    System(SystemAlbum),
    User(i64),
    Trash,
}

pub struct SidebarView;

impl SidebarView {
    pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        if this.sidebar_collapsed {
            return div()
                .w(px(48.0))
                .h_full()
                .bg(rgb(0x121215))
                .border_r_1()
                .border_color(rgb(0x27272a))
                .flex()
                .flex_col()
                .items_center()
                .py_4()
                .gap_4()
                .child(
                    div()
                        .id("expand-sidebar-btn")
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded_md()
                        .bg(rgb(0x1e1e24))
                        .hover(|s| s.bg(rgb(0x27272a)))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(rgb(0xa1a1aa))
                        .child("⊞")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.toggle_sidebar(cx);
                        })),
                )
                .into_any_element();
        }

        let engine = this.grid_state.as_ref().map(|s| &s.engine);
        let total_count = engine.map(|e| e.total_count()).unwrap_or(0);
        let fav_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Favorites)).unwrap_or(0);
        let video_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Videos)).unwrap_or(0);
        let raw_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Raw)).unwrap_or(0);
        let screenshot_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Screenshots)).unwrap_or(0);
        let long_exp_count = engine.map(|e| e.query_system_album_count(SystemAlbum::LongExposures)).unwrap_or(0);
        let pano_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Panoramas)).unwrap_or(0);
        let burst_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Bursts)).unwrap_or(0);
        let places_count = engine.map(|e| e.query_system_album_count(SystemAlbum::Places)).unwrap_or(0);
        let trash_count = engine.map(|e| e.query_system_album_count(SystemAlbum::RecentlyDeleted)).unwrap_or(0);
        let user_albums = engine.map(|e| e.list_smart_albums()).unwrap_or_default();

        div()
            .w(px(230.0))
            .h_full()
            .bg(rgb(0x121215))
            .border_r_1()
            .border_color(rgb(0x27272a))
            .flex()
            .flex_col()
            .justify_between()
            .p_3()
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    // Sidebar Header
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .px_2()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x71717a))
                                    .child("LIBRARY"),
                            )
                            .child(
                                div()
                                    .id("collapse-sidebar-btn")
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .rounded_md()
                                    .hover(|s| s.bg(rgb(0x27272a)))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(0x71717a))
                                    .child("⊟")
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_sidebar(cx);
                                    })),
                            ),
                    )
                    // Library Navigation Items
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(render_nav_item(
                                "All Photos & Videos",
                                "🖼",
                                total_count,
                                this.selection == AlbumSelection::AllLibrary,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::AllLibrary, cx);
                                }),
                            )),
                    )
                    // System Smart Albums Section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .px_2()
                                    .pt_2()
                                    .pb_1()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0x71717a))
                                    .child("SMART ALBUMS"),
                            )
                            .child(render_nav_item(
                                "Favorites",
                                "♥",
                                fav_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Favorites),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Favorites), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Videos",
                                "▶",
                                video_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Videos),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Videos), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "RAW Media",
                                "📷",
                                raw_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Raw),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Raw), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Screenshots",
                                "💻",
                                screenshot_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Screenshots),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Screenshots), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Long Exposures",
                                "⏱",
                                long_exp_count,
                                this.selection == AlbumSelection::System(SystemAlbum::LongExposures),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::LongExposures), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Panoramas",
                                "🌄",
                                pano_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Panoramas),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Panoramas), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Bursts",
                                "🥞",
                                burst_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Bursts),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Bursts), cx);
                                }),
                            ))
                            .child(render_nav_item(
                                "Places",
                                "📍",
                                places_count,
                                this.selection == AlbumSelection::System(SystemAlbum::Places),
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.select_album(AlbumSelection::System(SystemAlbum::Places), cx);
                                }),
                            )),
                    )
                    // User Defined Smart Albums Section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .px_2()
                                    .pt_2()
                                    .pb_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0x71717a))
                                            .child("MY ALBUMS"),
                                    )
                                    .child(
                                        div()
                                            .id("new-smart-album-btn")
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .hover(|s| s.bg(rgb(0x27272a)))
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(rgb(0x38bdf8))
                                            .child("+ New")
                                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                                this.create_new_smart_album(cx);
                                            })),
                                    ),
                            )
                            .children(user_albums.into_iter().map(|album| {
                                let album_id = album.id;
                                let is_active = this.selection == AlbumSelection::User(album_id);
                                let count = engine.map(|e| e.query_smart_album_count(&album)).unwrap_or(0);
                                render_nav_item(
                                    &album.title,
                                    "📁",
                                    count,
                                    is_active,
                                    cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                        this.select_album(AlbumSelection::User(album_id), cx);
                                    }),
                                )
                            })),
                    ),
            )
            // Trash Section at bottom
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(rgb(0x27272a))
                    .child(render_nav_item(
                        "Recently Deleted",
                        "🗑",
                        trash_count,
                        this.selection == AlbumSelection::Trash,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.select_album(AlbumSelection::Trash, cx);
                        }),
                    )),
            )
            .into_any_element()
    }
}

fn render_nav_item(
    label: &str,
    icon: &str,
    count: usize,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg_color = if is_active { rgb(0x2563eb) } else { rgb(0x121215) };
    let text_color = if is_active { rgb(0xffffff) } else { rgb(0xd4d4d8) };
    let badge_bg = if is_active { rgb(0x1d4ed8) } else { rgb(0x1e1e24) };

    div()
        .id(ElementId::Name(label.to_string().into()))
        .px_2p5()
        .py_1p5()
        .rounded_md()
        .bg(bg_color)
        .hover(move |s| {
            if is_active {
                s.bg(rgb(0x1d4ed8))
            } else {
                s.bg(rgb(0x1e1e24))
            }
        })
        .cursor_pointer()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .on_click(on_click)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(div().text_xs().child(icon.to_string()))
                .child(
                    div()
                        .text_xs()
                        .font_weight(if is_active { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                        .text_color(text_color)
                        .child(label.to_string()),
                ),
        )
        .child(
            div()
                .px_1p5()
                .py_0p5()
                .rounded_full()
                .bg(badge_bg)
                .text_xs()
                .text_color(rgb(0xa1a1aa))
                .child(count.to_string()),
        )
}
