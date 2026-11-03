use gpui::*;
use lumina_core::{SmartAlbum, SystemAlbum};

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
    let downloaded_count = this
        .system_album_counts
        .get(&SystemAlbum::Downloaded)
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
    let missing_count = this
        .system_album_counts
        .get(&SystemAlbum::Missing)
        .copied()
        .unwrap_or(0);

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
                .pt(px(16.0))
                .pb_1()
                .child(theme::section_label("LIBRARY"))
                .child(
                    div()
                        .id("change-folder-btn")
                        .debug_selector(|| "switch-library".to_string())
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(theme::RADIUS_SM)
                        .hover(|s| s.bg(theme::bg_control()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .tooltip(theme::tooltip_label("Open Library… (⌘O)"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::Folder,
                            px(12.0),
                            theme::text_subtle(),
                        ))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.pick_folder_dialog(cx);
                        })),
                ),
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
        // Only once something was downloaded from a link.
        .children((downloaded_count > 0).then(|| {
            render_nav_item(
                "Downloaded",
                downloaded_count,
                this.selection == AlbumSelection::System(SystemAlbum::Downloaded),
                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.select_album(AlbumSelection::System(SystemAlbum::Downloaded), cx);
                }),
            )
        }))
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
            "Duplicates",
            this.duplicate_items_count,
            this.selection == AlbumSelection::Duplicates,
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.select_album(AlbumSelection::Duplicates, cx);
            }),
        ))
        // Only while something is missing: there is nothing to do otherwise.
        .children((missing_count > 0).then(|| {
            render_nav_item(
                "Missing",
                missing_count,
                this.selection == AlbumSelection::System(SystemAlbum::Missing),
                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.select_album(AlbumSelection::System(SystemAlbum::Missing), cx);
                }),
            )
        }))
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
}

fn render_user_smart_album_row(
    _this: &LuminaApp,
    album: &SmartAlbum,
    count: usize,
    is_active: bool,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let album_id = album.id;
    let edit_album = album.clone();
    div()
        .id(("smart-album-row", album_id as u64))
        .group("smart-album-row")
        .debug_selector(move || format!("smart-album-row-{album_id}"))
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .h(px(26.0))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
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
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme::TEXT_BODY)
                .text_color(if is_active {
                    theme::selection_fg()
                } else {
                    theme::text_row()
                })
                .child(album.title.clone()),
        )
        .child(
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if is_active {
                    theme::selection_count()
                } else {
                    theme::text_subtle()
                })
                .child(count.to_string()),
        )
        .child(
            div()
                .id(("smart-album-edit", album_id as u64))
                .debug_selector(move || format!("smart-album-edit-{album_id}"))
                // Row actions appear on hover only (v3 sidebar).
                .opacity(0.0)
                .group_hover("smart-album-row", |s| s.opacity(1.0))
                .px_1()
                .rounded(theme::RADIUS_SM)
                .cursor_pointer()
                .tooltip(theme::tooltip_label("Edit rules"))
                .child(crate::icons::render_icon(
                    crate::icons::Icon::Pencil,
                    px(11.0),
                    theme::text_subtle(),
                ))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.open_album_builder_for_edit(edit_album.clone(), cx);
                })),
        )
        .child(
            div()
                .id(("smart-album-delete", album_id as u64))
                .debug_selector(move || format!("smart-album-delete-{album_id}"))
                // Row actions appear on hover only (v3 sidebar).
                .opacity(0.0)
                .group_hover("smart-album-row", |s| s.opacity(1.0))
                .px_1()
                .rounded(theme::RADIUS_SM)
                .cursor_pointer()
                .tooltip(theme::tooltip_label("Delete smart album"))
                .child(crate::icons::render_icon(
                    crate::icons::Icon::X,
                    px(11.0),
                    theme::text_subtle(),
                ))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    cx.stop_propagation();
                    this.prompt_delete_smart_album(album_id, cx);
                })),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.select_album(AlbumSelection::User(album_id), cx);
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
                .pt(px(16.0))
                .pb_1()
                .child(theme::section_label("SMART ALBUMS"))
                .child(
                    div()
                        .id("new-smart-album-btn")
                        .debug_selector(|| "new-smart-album".to_string())
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(theme::RADIUS_SM)
                        .hover(|s| s.bg(theme::bg_control()).text_color(theme::text()))
                        .flex()
                        .justify_center()
                        .items_center()
                        .cursor_pointer()
                        .tooltip(theme::tooltip_label("New smart album (⌥⌘N)"))
                        .child(crate::icons::render_icon(
                            crate::icons::Icon::Plus,
                            px(12.0),
                            theme::text_subtle(),
                        ))
                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.open_album_builder(cx);
                        })),
                ),
        )
        .children(if user_albums.is_empty() {
            if this.grid_state.is_none() {
                vec![render_nav_item(
                    "Fast Glass",
                    0,
                    false,
                    cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_album_builder(cx);
                    }),
                )
                .into_any_element()]
            } else {
                Vec::new()
            }
        } else {
            user_albums
                .iter()
                .map(|album| {
                    let is_active = this.selection == AlbumSelection::User(album.id);
                    let count = this.user_album_counts.get(&album.id).copied().unwrap_or(0);
                    render_user_smart_album_row(this, album, count, is_active, cx)
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        // Untagged is a built-in smart album (v3 sidebar).
        .child(render_nav_item(
            "Untagged",
            this.untagged_count,
            this.selection == AlbumSelection::Untagged,
            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                if this.selection == AlbumSelection::Untagged {
                    this.select_album(AlbumSelection::AllLibrary, cx);
                } else {
                    this.select_album(AlbumSelection::Untagged, cx);
                }
            }),
        ))
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
                .debug_selector(|| "sidebar-tags-header".to_string())
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_2()
                .pt(px(16.0))
                .pb_1()
                .child(
                    theme::section_label("TAGS"),
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
                            .debug_selector(|| "sidebar-tag-rename".to_string())
                            .px_2()
                            .py_1()
                            .rounded(theme::RADIUS_MD)
                            .bg(theme::bg_pane())
                            .border_1()
                            .border_color(theme::focus_ring())
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            // A long name shrinks and clips on the left, so the
                            // caret at its end and the buttons stay in view.
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .flex()
                                    .justify_end()
                                    .child(
                                        div()
                                            .flex_none()
                                            .whitespace_nowrap()
                                            .text_size(theme::TEXT_BODY)
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(theme::text())
                                            .child(format!("{}|", this.editing_tag_buffer)),
                                    ),
                            )
                            .child(
                                div()
                                    .debug_selector(|| "sidebar-tag-rename-buttons".to_string())
                                    .flex_none()
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
                                            .rounded(theme::RADIUS_SM)
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
                                            .rounded(theme::RADIUS_SM)
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
                        let tag_for_debug = tag_str.clone();

                        div()
                            .id(ElementId::Name(
                                format!("sidebar-tag-item-{}", tag_str).into(),
                            ))
                            .tooltip(theme::tooltip_label(
                                "Click to filter · double-click to rename",
                            ))
                            .px_2()
                            .h(px(26.0))
                            .rounded(theme::RADIUS_MD)
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
                                    .debug_selector(move || {
                                        format!("sidebar-tag-filter-{}", tag_for_debug)
                                    })
                                    .flex_1()
                                    .min_w_0()
                                    .cursor_pointer()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div().flex_none().size(px(7.0)).rounded_full().bg(dot_color),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(theme::TEXT_BODY)
                                            .font_weight(if is_active {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .text_color(if is_active {
                                                theme::selection_fg()
                                            } else {
                                                theme::text_row()
                                            })
                                            .child(tag_str.clone()),
                                    )
                                    .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
                                        if e.click_count() >= 2 {
                                            this.start_tag_rename(tag_for_select.clone(), cx);
                                        } else if matches!(&this.selection, AlbumSelection::Tag(t) if t == &tag_for_select) {
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
                                        theme::selection_count()
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
    let label_text = label.to_string();
    let debug_label = label_text.clone();
    let bg_color = if is_active {
        theme::selection_bg()
    } else {
        theme::bg_app()
    };
    let text_color = if is_active {
        theme::selection_fg()
    } else {
        theme::text_row()
    };
    let count_color = if is_active {
        theme::selection_count()
    } else {
        theme::text_subtle()
    };

    div()
        .id(ElementId::Name(label_text.clone().into()))
        .debug_selector(move || format!("sidebar-nav-{debug_label}"))
        .px_2()
        .h(px(26.0))
        .rounded(theme::RADIUS_MD)
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
                .text_size(theme::TEXT_BODY)
                .font_weight(if is_active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(text_color)
                .child(label_text),
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
