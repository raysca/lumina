use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::{AlbumSelection, LuminaApp};
use gpui::*;
use lumina_core::MediaItem;

#[derive(Clone, Debug)]
pub struct ContextMenuState {
    pub position: Point<Pixels>,
    pub item: MediaItem,
}

pub struct MediaContextMenu;

impl MediaContextMenu {
    pub fn render(
        app: &LuminaApp,
        window: &Window,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let state = match app.context_menu_state.as_ref() {
            Some(s) => s.clone(),
            None => return Empty.into_any_element(),
        };

        let item = state.item.clone();
        let is_trash = app.selection == AlbumSelection::Trash;
        let is_fav = item.is_favorite;
        let is_video = item.media_type == lumina_core::MediaType::Video;
        let path = item.path.clone();

        let raw_x: f32 = state.position.x.into();
        let raw_y: f32 = state.position.y.into();
        let menu_w = 230.0;
        let menu_h = 380.0;
        let win_size = window.viewport_size();
        let win_w: f32 = win_size.width.into();
        let win_h: f32 = win_size.height.into();

        let x = (raw_x.min(win_w - menu_w - 8.0)).max(8.0);
        let y = (raw_y.min(win_h - menu_h - 8.0)).max(8.0);

        div()
            .id("context-menu-layer")
            .absolute()
            .inset_0()
            .on_scroll_wheel(cx.listener(|this, _e: &ScrollWheelEvent, _w, cx| {
                this.close_context_menu(cx);
            }))
            // Dismiss on background click
            .child(
                div()
                    .id("context-menu-dismiss-backdrop")
                    .absolute()
                    .inset_0()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            this.close_context_menu(cx);
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            this.close_context_menu(cx);
                        }),
                    ),
            )
            // Menu Popover Container
            .child(
                div()
                    .id("media-context-menu-popover")
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(230.0))
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_lg()
                    .shadow_2xl()
                    .p_1()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    // 1. Open in Viewer
                    .child({
                        let item_for_viewer = item.clone();
                        Self::render_menu_item(
                            "preview-menu-btn",
                            "Open in Viewer",
                            Some("Space"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_viewer(item_for_viewer.clone(), cx);
                            }),
                        )
                    })
                    // 2. Play Video (if video)
                    .children(is_video.then(|| {
                        let item_for_vid = item.clone();
                        Self::render_menu_item(
                            "play-video-menu-btn",
                            "Play Video",
                            None,
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_viewer(item_for_vid.clone(), cx);
                            }),
                        )
                    }))
                    // 3. Play in Picture-in-Picture
                    .child(Self::render_menu_item(
                        "pip-menu-btn",
                        "Play in Picture-in-Picture",
                        Some("⌥P"),
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.open_floating_pip(cx);
                        }),
                    ))
                    // 4. Get Info
                    .child(Self::render_menu_item(
                        "inspect-menu-btn",
                        "Get Info",
                        Some("⌘I"),
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.inspector_open = true;
                            cx.notify();
                        }),
                    ))
                    // Divider
                    .child(Self::render_divider())
                    // 5. Toggle Favorite (only if not in Trash)
                    .child(if !is_trash {
                        Self::render_menu_item(
                            "fav-menu-btn",
                            if is_fav { "Unfavorite" } else { "Favorite" },
                            Some("F"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.batch_toggle_favorite(cx);
                            }),
                        )
                        .into_any_element()
                    } else {
                        Empty.into_any_element()
                    })
                    // 6. Interactive Rating Row
                    .child(
                        div()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(div().text_xs().text_color(theme::text()).child("Rating"))
                            .child(div().flex().flex_row().items_center().gap_1().children(
                                (1..=5).map(|star_val| {
                                    let current_rating = app.get_item_rating(&item);
                                    let is_filled = star_val <= current_rating;
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "context-rating-star".into(),
                                            star_val as u64,
                                        ))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(if is_filled {
                                            theme::rating()
                                        } else {
                                            theme::text_disabled()
                                        })
                                        .child(render_icon(
                                            if is_filled {
                                                Icon::StarFilled
                                            } else {
                                                Icon::Star
                                            },
                                            px(12.0),
                                            if is_filled {
                                                theme::rating()
                                            } else {
                                                theme::text_disabled()
                                            },
                                        ))
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.set_rating_selected(star_val, cx);
                                            },
                                        ))
                                }),
                            )),
                    )
                    // 7. Interactive Color Label Row
                    .child(
                        div()
                            .px_2p5()
                            .py_1()
                            .rounded_md()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(div().text_xs().text_color(theme::text()).child("Label"))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child({
                                        let current_label = app
                                            .item_color_labels
                                            .get(&item.id)
                                            .copied()
                                            .unwrap_or(0);
                                        div()
                                            .id("context-label-0")
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(if current_label == 0 {
                                                theme::text()
                                            } else {
                                                theme::text_disabled()
                                            })
                                            .child("⊘")
                                            .on_click(cx.listener(
                                                move |this, _e: &ClickEvent, _w, cx| {
                                                    this.set_color_label_selected(0, cx);
                                                },
                                            ))
                                    })
                                    .children(
                                        [
                                            (1u8, theme::label_red()),
                                            (2, theme::label_orange()),
                                            (3, theme::label_yellow()),
                                            (4, theme::label_green()),
                                            (5, theme::label_blue()),
                                            (6, theme::label_purple()),
                                        ]
                                        .into_iter()
                                        .map(
                                            |(label_idx, color)| {
                                                let current_label = app
                                                    .item_color_labels
                                                    .get(&item.id)
                                                    .copied()
                                                    .unwrap_or(0);
                                                let is_selected = current_label == label_idx;
                                                let mut dot = div()
                                                    .id(ElementId::NamedInteger(
                                                        "context-label-dot".into(),
                                                        label_idx as u64,
                                                    ))
                                                    .size(px(10.0))
                                                    .rounded_full()
                                                    .bg(color)
                                                    .cursor_pointer();
                                                if is_selected {
                                                    dot =
                                                        dot.border_1().border_color(theme::text());
                                                }
                                                dot.on_click(cx.listener(
                                                    move |this, _e: &ClickEvent, _w, cx| {
                                                        this.set_color_label_selected(
                                                            label_idx, cx,
                                                        );
                                                    },
                                                ))
                                            },
                                        ),
                                    ),
                            ),
                    )
                    // Divider
                    .child(Self::render_divider())
                    // 8. Tags...
                    .child(Self::render_menu_item(
                        "tags-menu-btn",
                        "Tags...",
                        Some("T"),
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.open_tag_manager(cx);
                        }),
                    ))
                    // 9. Add to Album
                    .child(Self::render_submenu_item(
                        "add-album-menu-btn",
                        "Add to Album",
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.open_album_builder(cx);
                        }),
                    ))
                    // 10. New Album from Selection
                    .child(Self::render_menu_item(
                        "new-album-sel-menu-btn",
                        "New Album from Selection",
                        None,
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.open_album_builder(cx);
                        }),
                    ))
                    // Divider
                    .child(Self::render_divider())
                    // 11. Copy File Path
                    .child({
                        let path_for_copy = item.path.to_string_lossy().to_string();
                        Self::render_menu_item(
                            "copy-path-menu-btn",
                            "Copy File Path",
                            Some("⌥⌘C"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.copy_to_clipboard(&path_for_copy, cx);
                            }),
                        )
                    })
                    // 12. Reveal in Finder
                    .child({
                        let path_for_reveal = path.clone();
                        Self::render_menu_item(
                            "reveal-menu-btn",
                            "Reveal in Finder",
                            Some("⇧⌘R"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.reveal_item_in_finder(&path_for_reveal);
                            }),
                        )
                    })
                    // 13. Open With
                    .child({
                        let path_for_open = path.clone();
                        Self::render_submenu_item(
                            "open-with-menu-btn",
                            "Open With",
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_item_in_default_app(&path_for_open);
                            }),
                        )
                    })
                    // Divider
                    .child(Self::render_divider())
                    // 14. Delete / Trash
                    .child(if is_trash {
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(Self::render_menu_item(
                                "restore-menu-btn",
                                "Restore from Trash",
                                None,
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_context_menu(cx);
                                    this.batch_trash_or_restore(cx);
                                }),
                            ))
                            .child(Self::render_menu_item(
                                "delete-perm-menu-btn",
                                "Delete Immediately...",
                                None,
                                true,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_context_menu(cx);
                                    this.prompt_delete_selected(cx);
                                }),
                            ))
                            .into_any_element()
                    } else {
                        Self::render_menu_item(
                            "trash-menu-btn",
                            "Delete...",
                            Some("⌘⌫"),
                            true,
                            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.batch_trash_or_restore(cx);
                            }),
                        )
                        .into_any_element()
                    }),
            )
            .into_any_element()
    }

    fn render_divider() -> impl IntoElement {
        div().h(px(1.0)).bg(theme::border()).my_1().mx_1()
    }

    fn render_menu_item(
        id_str: &'static str,
        label: &'static str,
        shortcut: Option<&'static str>,
        is_destructive: bool,
        on_click_handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let trailing = if let Some(sc) = shortcut {
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(theme::text_subtle())
                .child(sc)
                .into_any_element()
        } else {
            Empty.into_any_element()
        };
        Self::render_menu_row(id_str, label, trailing, is_destructive, on_click_handler)
    }

    /// A menu row that opens a submenu, marked with a chevron instead of a
    /// keyboard shortcut.
    fn render_submenu_item(
        id_str: &'static str,
        label: &'static str,
        is_destructive: bool,
        on_click_handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let trailing =
            render_icon(Icon::ChevronRight, px(12.0), theme::text_subtle()).into_any_element();
        Self::render_menu_row(id_str, label, trailing, is_destructive, on_click_handler)
    }

    fn render_menu_row(
        id_str: &'static str,
        label: &'static str,
        trailing: AnyElement,
        is_destructive: bool,
        on_click_handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id_str)
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .cursor_pointer()
            .hover(move |s| {
                if is_destructive {
                    s.bg(theme::danger_bg()).text_color(theme::danger_text())
                } else {
                    s.bg(theme::bg_control_hover()).text_color(theme::text())
                }
            })
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_xs()
                    .text_color(if is_destructive {
                        theme::danger_text()
                    } else {
                        theme::text()
                    })
                    .child(label),
            )
            .child(trailing)
            .on_click(on_click_handler)
    }
}
