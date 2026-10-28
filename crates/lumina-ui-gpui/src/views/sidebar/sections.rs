use gpui::*;
use lumina_core::SystemAlbum;

use super::{tag_color, AlbumSelection};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub(super) fn render_library_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let total_count = this.total_library_count;
    let fav_count = this
        .system_album_counts
        .get(&SystemAlbum::Favorites)
        .copied()
        .unwrap_or(0);
    let video_count = this
        .system_album_counts
        .get(&SystemAlbum::Videos)
        .copied()
        .unwrap_or(0);
    let screenshot_count = this
        .system_album_counts
        .get(&SystemAlbum::Screenshots)
        .copied()
        .unwrap_or(0);
    let places_count = this
        .system_album_counts
        .get(&SystemAlbum::Places)
        .copied()
        .unwrap_or(0);
    let trash_count = this
        .system_album_counts
        .get(&SystemAlbum::RecentlyDeleted)
        .copied()
        .unwrap_or(0);

    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_subtle())
                .child("LIBRARY"),
        )
        .child(render_nav_item(
            "All Photos & Videos",
            if this.grid_state.is_some() {
                total_count
            } else {
                22
            },
            this.selection == AlbumSelection::AllLibrary,
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.set_active_year(None, cx);
                this.select_album(AlbumSelection::AllLibrary, cx);
            }),
        ))
        .child(render_nav_item(
            "Favorites",
            if this.grid_state.is_some() {
                fav_count
            } else {
                3
            },
            this.selection == AlbumSelection::System(SystemAlbum::Favorites),
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::System(SystemAlbum::Favorites), cx);
            }),
        ))
        .child(render_nav_item(
            "Videos",
            if this.grid_state.is_some() {
                video_count
            } else {
                7
            },
            this.selection == AlbumSelection::System(SystemAlbum::Videos),
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::System(SystemAlbum::Videos), cx);
            }),
        ))
        .child(render_nav_item(
            "Screenshots",
            if this.grid_state.is_some() {
                screenshot_count
            } else {
                2
            },
            this.selection == AlbumSelection::System(SystemAlbum::Screenshots),
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::System(SystemAlbum::Screenshots), cx);
            }),
        ))
        .child(render_nav_item(
            "Places",
            places_count,
            this.selection == AlbumSelection::System(SystemAlbum::Places),
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::System(SystemAlbum::Places), cx);
            }),
        ))
        .child(render_nav_item(
            "Recently Deleted",
            if this.grid_state.is_some() {
                trash_count
            } else {
                1
            },
            this.selection == AlbumSelection::Trash,
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::Trash, cx);
            }),
        ))
        .child(
            if this.selection == AlbumSelection::Trash && trash_count > 0 {
                div()
                    .flex()
                    .flex_row()
                    .gap_1p5()
                    .px_1()
                    .pt_1()
                    .child(
                        div()
                            .id("sidebar-empty-trash-btn")
                            .flex_1()
                            .py_1()
                            .rounded_md()
                            .bg(theme::danger_bg())
                            .hover(|s| s.bg(theme::danger_bg_hover()))
                            .cursor_pointer()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text())
                            .child("Empty Trash")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.prompt_empty_trash(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("sidebar-restore-all-btn")
                            .flex_1()
                            .py_1()
                            .rounded_md()
                            .bg(theme::selection_bg())
                            .hover(|s| s.bg(theme::selection_bg_hover()))
                            .cursor_pointer()
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::accent())
                            .child("Restore All")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.restore_all_trash(cx);
                            })),
                    )
                    .into_any_element()
            } else {
                Empty.into_any_element()
            },
        )
}

pub(super) fn render_events_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let years: Vec<(i32, usize)> = if this.grid_state.is_some() {
        this.timeline_year_buckets.clone()
    } else {
        vec![(2026, 2), (2024, 3), (2023, 9), (2022, 5)]
    };

    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .px_2()
                .pt_1()
                .pb_0p5()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_subtle())
                .child("EVENTS"),
        )
        .children(years.into_iter().map(|(year, count)| {
            let is_active = this.active_year == Some(year);
            render_nav_item(
                &year.to_string(),
                count,
                is_active,
                cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    if this.active_year == Some(year) {
                        this.set_active_year(None, cx);
                    } else {
                        this.set_active_year(Some(year), cx);
                    }
                }),
            )
        }))
}

pub(super) fn render_smart_albums_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let user_albums = &this.user_smart_albums;

    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_2()
                .pt_1()
                .pb_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("SMART ALBUMS"),
                )
                .child(
                    div()
                        .id("new-smart-album-btn")
                        .debug_selector(|| "new-smart-album".to_string())
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded_sm()
                        .hover(|s| s.bg(theme::bg_control()).text_color(theme::text()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .text_sm()
                        .text_color(theme::text_subtle())
                        .child("+")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_album_builder(cx);
                        })),
                ),
        )
        .children(if user_albums.is_empty() {
            if this.grid_state.is_none() {
                vec![
                    render_nav_item(
                        "Fast Glass",
                        0,
                        false,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_album_builder(cx);
                        }),
                    )
                    .into_any_element(),
                    render_nav_item(
                        "Untagged",
                        18,
                        false,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_album_builder(cx);
                        }),
                    )
                    .into_any_element(),
                ]
            } else {
                Vec::new()
            }
        } else {
            user_albums
                .iter()
                .map(|album| {
                    let album_id = album.id;
                    let is_active = this.selection == AlbumSelection::User(album_id);
                    let count = this.user_album_counts.get(&album_id).copied().unwrap_or(0);
                    render_nav_item(
                        &album.title,
                        count,
                        is_active,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.select_album(AlbumSelection::User(album_id), cx);
                        }),
                    )
                    .into_any_element()
                })
                .collect::<Vec<_>>()
        })
}

pub(super) fn render_tags_section(
    this: &LuminaApp,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let tags: Vec<(String, usize)> = if this.grid_state.is_some() {
        this.library_tag_counts.clone()
    } else {
        vec![
            ("family".to_string(), 2),
            ("travel".to_string(), 4),
            ("street".to_string(), 2),
        ]
    };

    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_2()
                .pt_1()
                .pb_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme::text_subtle())
                        .child("TAGS"),
                )
                .child(
                    div()
                        .id("open-tag-manager-sidebar-btn")
                        .debug_selector(|| "open-tag-manager".to_string())
                        .cursor_pointer()
                        .text_xs()
                        .text_color(theme::accent())
                        .hover(|s| s.text_color(theme::accent_hover()))
                        .child("Manage...")
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_tag_manager(cx);
                        })),
                ),
        )
        .children(
            tags.iter()
                .map(|(tag_name, count)| {
                    let tag_str = tag_name.clone();
                    let is_editing = this.editing_tag.as_ref() == Some(&tag_str);
                    let is_active =
                        matches!(&this.selection, AlbumSelection::Tag(t) if t == &tag_str);
                    let tag_count = *count;
                    let tag_for_select = tag_str.clone();

                    if is_editing {
                        div()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .bg(theme::bg_pane())
                            .border_1()
                            .border_color(theme::accent())
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(format!("{}|", this.editing_tag_buffer)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(ElementId::Name(
                                                format!("save-tag-rename-{}", tag_str).into(),
                                            ))
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme::accent())
                                            .hover(|s| {
                                                s.bg(theme::accent_hover())
                                                    .text_color(theme::accent_fg())
                                            })
                                            .cursor_pointer()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme::accent_fg())
                                            .child(render_icon(
                                                Icon::Check,
                                                px(10.0),
                                                theme::accent_fg(),
                                            ))
                                            .on_click(cx.listener(
                                                |this, _e: &ClickEvent, _w, cx| {
                                                    this.commit_tag_rename(cx);
                                                },
                                            )),
                                    )
                                    .child(
                                        div()
                                            .id(ElementId::Name(
                                                format!("cancel-tag-rename-{}", tag_str).into(),
                                            ))
                                            .px_1p5()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(theme::bg_control())
                                            .hover(|s| s.bg(theme::bg_control_hover()))
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(theme::text_muted())
                                            .child(render_icon(Icon::X, px(10.0), theme::text_muted()))
                                            .on_click(cx.listener(
                                                |this, _e: &ClickEvent, _w, cx| {
                                                    this.cancel_tag_rename(cx);
                                                },
                                            )),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        let dot_color = tag_color(&tag_str);

                        div()
                            .id(ElementId::Name(
                                format!("sidebar-tag-item-{}", tag_str).into(),
                            ))
                            .px_2()
                            .py_1p5()
                            .rounded_md()
                            .bg(if is_active {
                                theme::selection_bg()
                            } else {
                                theme::bg_app()
                            })
                            .hover(move |s| {
                                if is_active {
                                    s.bg(theme::selection_bg_hover())
                                } else {
                                    s.bg(theme::bg_raised())
                                }
                            })
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .id(ElementId::Name(
                                        format!("sidebar-tag-filter-{}", tag_str).into(),
                                    ))
                                    .flex_1()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(div().size(px(7.0)).rounded_full().bg(dot_color))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(if is_active {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(if is_active {
                                                theme::accent()
                                            } else {
                                                theme::text()
                                            })
                                            .child(tag_str.clone()),
                                    )
                                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                        if matches!(&this.selection, AlbumSelection::Tag(t) if t == &tag_for_select) {
                                            this.select_album(AlbumSelection::AllLibrary, cx);
                                        } else {
                                            this.select_album(
                                                AlbumSelection::Tag(tag_for_select.clone()),
                                                cx,
                                            );
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family(theme::FONT_MONO)
                                    .text_color(if is_active {
                                        theme::accent()
                                    } else {
                                        theme::text_subtle()
                                    })
                                    .child(tag_count.to_string()),
                            )
                            .into_any_element()
                    }
                })
                .collect::<Vec<_>>(),
        )
}

fn render_nav_item(
    label: &str,
    count: usize,
    is_active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let bg_color = if is_active {
        theme::selection_bg()
    } else {
        theme::bg_app()
    };
    let text_color = if is_active {
        theme::accent()
    } else {
        theme::text()
    };
    let count_color = if is_active {
        theme::accent()
    } else {
        theme::text_subtle()
    };

    div()
        .id(ElementId::Name(label.to_string().into()))
        .px_2()
        .py_1p5()
        .rounded_md()
        .bg(bg_color)
        .hover(move |s| {
            if is_active {
                s.bg(theme::selection_bg_hover())
            } else {
                s.bg(theme::bg_raised())
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
                .text_xs()
                .font_weight(if is_active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(text_color)
                .child(label.to_string()),
        )
        .child(if count > 0 {
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(count_color)
                .child(count.to_string())
                .into_any_element()
        } else {
            Empty.into_any_element()
        })
}
