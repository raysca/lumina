use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::viewer_state::ViewerRotation;
use crate::{AlbumSelection, LuminaApp};
use gpui::*;
use lumina_core::MediaItem;

#[derive(Clone, Debug)]
pub struct ContextMenuState {
    pub position: Point<Pixels>,
    pub item: MediaItem,
    /// Row under the pointer. GPUI resolves text colour at layout, before
    /// hover styles apply, so the hovered row's dark-on-amber text is
    /// chosen from this state at render instead.
    pub hovered: Option<&'static str>,
}

/// What every row needs to paint its hover state.
struct RowHover {
    hovered: Option<&'static str>,
    app: WeakEntity<LuminaApp>,
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
        let hover = RowHover {
            hovered: state.hovered,
            app: cx.entity().downgrade(),
        };
        let is_trash = app.selection == AlbumSelection::Trash;
        let in_album = matches!(app.selection, AlbumSelection::Album(_));
        let is_fav = item.is_favorite;
        let is_video = item.media_type == lumina_core::MediaType::Video;
        let in_viewer = app.is_fullscreen_viewer;
        let path = item.path.clone();
        let is_missing = app.missing_items.contains(&item.id);

        let raw_x: f32 = state.position.x.into();
        let raw_y: f32 = state.position.y.into();
        let menu_w = 256.0;
        let menu_h = 530.0;
        let win_size = window.viewport_size();
        let win_w: f32 = win_size.width.into();
        let win_h: f32 = win_size.height.into();

        let x = (raw_x.min(win_w - menu_w - 8.0)).max(8.0);
        let y = (raw_y.min(win_h - menu_h - 8.0)).max(8.0);

        div()
            .id("context-menu-layer")
            .absolute()
            .inset_0()
            .on_scroll(cx.listener(|this, _e: &ScrollEvent, _w, cx| {
                this.close_context_menu(cx);
            }))
            // Dismiss on background click
            .child(
                div()
                    .id("context-menu-dismiss-backdrop")
                    .debug_selector(|| "context-menu-backdrop".to_string())
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
                    .debug_selector(|| "context-menu".to_string())
                    .occlude()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(menu_w))
                    .max_h(px((win_h - 16.0).max(120.0)))
                    .overflow_y_scroll()
                    .bg(theme::bg_hud())
                    .border_1()
                    .border_color(theme::border_strong())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .p_1()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    // 1. Open in Viewer (not inside the viewer itself)
                    .children((!in_viewer).then(|| {
                        let item_for_viewer = item.clone();
                        Self::render_menu_item(
                            &hover,
                            "preview-menu-btn",
                            "Open in Viewer",
                            Some("Space"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_viewer(item_for_viewer.clone(), cx);
                            }),
                        )
                    }))
                    // 2. Play Video (if video)
                    .children((is_video && !in_viewer).then(|| {
                        let item_for_vid = item.clone();
                        Self::render_menu_item(
                            &hover,
                            "play-video-menu-btn",
                            "Play Video",
                            Some("↵"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_viewer(item_for_vid.clone(), cx);
                            }),
                        )
                    }))
                    // 3. Play in Picture-in-Picture (videos only; acts on the
                    // right-clicked item, not the selection)
                    .children(is_video.then(|| {
                        let item_for_pip = item.clone();
                        Self::render_menu_item(
                            &hover,
                            "pip-menu-btn",
                            "Play in Picture-in-Picture",
                            Some("⌥P"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.play_in_pip(item_for_pip.clone(), cx);
                            }),
                        )
                    }))
                    // 4. Get Info
                    .child(Self::render_menu_item(
                        &hover,
                        "inspect-menu-btn",
                        "Get Info",
                        Some("⌘I"),
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.open_get_info_at(crate::views::inspector::InspectorTab::Info, cx);
                        }),
                    ))
                    // Divider
                    .child(Self::render_divider())
                    // 5. Toggle Favorite (only if not in Trash)
                    .child(if !is_trash {
                        Self::render_menu_item(
                            &hover,
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
                            .rounded(theme::RADIUS_SM)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(div().text_xs().text_color(theme::text()).child("Rating"))
                            .child(div().flex().flex_row().items_center().children((1..=5).map(
                                |star_val| {
                                    let current_rating = app.get_item_rating(&item);
                                    let star_pop = app.mark_pop.star(Some(item.id));
                                    let is_filled = star_val <= current_rating;
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "context-rating-star".into(),
                                            star_val as u64,
                                        ))
                                        .debug_selector(move || {
                                            format!("context-rating-star-{star_val}")
                                        })
                                        .size(px(20.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .child(crate::motion::pop(
                                            render_icon(
                                                if is_filled {
                                                    Icon::StarFilled
                                                } else {
                                                    Icon::Star
                                                },
                                                px(12.0),
                                                if is_filled {
                                                    theme::rating()
                                                } else {
                                                    theme::rating_unset()
                                                },
                                            ),
                                            star_pop
                                                .clone()
                                                .filter(|(s, _)| *s == star_val)
                                                .map(|(_, id)| id),
                                        ))
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.set_rating_selected(star_val, cx);
                                            },
                                        ))
                                },
                            ))),
                    )
                    // 7. Interactive Color Label Row
                    .child(
                        div()
                            .px_2p5()
                            .py_1()
                            .rounded(theme::RADIUS_SM)
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(div().text_xs().text_color(theme::text()).child("Label"))
                            .child({
                                let current_label =
                                    app.item_color_labels.get(&item.id).copied().unwrap_or(0);
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(2.0))
                                    .children(theme::label_options().into_iter().map(|label| {
                                        let label_idx = label.0;
                                        theme::label_swatch(
                                            "context-label",
                                            label,
                                            current_label == label_idx,
                                        )
                                        .on_click(
                                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                                this.set_color_label_selected(label_idx, cx);
                                            }),
                                        )
                                    }))
                            }),
                    )
                    // Divider
                    .child(Self::render_divider())
                    // 8. Tags...
                    .child(Self::render_menu_item(
                        &hover,
                        "tags-menu-btn",
                        "Tags…",
                        Some("T"),
                        false,
                        cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.begin_tagging_selection(cx);
                        }),
                    ))
                    // 10. Add to Album…, Remove from Album (in an album), New
                    // Album with Selection
                    .children((!is_trash).then(|| {
                        Self::render_menu_item(
                            &hover,
                            "context-add-to-album",
                            crate::i18n::t("command.add_to_album"),
                            Some("⇧⌘A"),
                            false,
                            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.open_album_picker(cx);
                            }),
                        )
                    }))
                    .children(in_album.then(|| {
                        Self::render_menu_item(
                            &hover,
                            "context-remove-from-album",
                            crate::i18n::t("command.remove_from_album"),
                            // ⌫ removes only in the grid.
                            (!in_viewer).then_some("⌫"),
                            false,
                            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.remove_selection_from_album(cx);
                            }),
                        )
                    }))
                    .child(Self::render_menu_item(
                        &hover,
                        "context-new-album-with-selection",
                        crate::i18n::t("command.new_album_with_selection"),
                        None,
                        false,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.new_album_with_selection(cx);
                        }),
                    ))
                    // 11. Export...
                    .child(Self::render_menu_item(
                        &hover,
                        "export-menu-btn",
                        "Export…",
                        None,
                        false,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.export_pick_folder(cx);
                        }),
                    ))
                    // 12. Import XMP metadata
                    .child(Self::render_menu_item(
                        &hover,
                        "import-xmp-menu-btn",
                        "Import XMP Metadata",
                        None,
                        false,
                        cx.listener(|this, _e: &ClickEvent, _w, cx| {
                            this.close_context_menu(cx);
                            this.import_sidecars_selected(cx);
                        }),
                    ))
                    // Divider
                    .child(Self::render_divider())
                    // 13. Copy File Path
                    .child({
                        let path_for_copy = item.path.to_string_lossy().to_string();
                        Self::render_menu_item(
                            &hover,
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
                    // Missing file: point it at where it went.
                    .children(is_missing.then(|| {
                        Self::render_menu_item(
                            &hover,
                            "missing-locate",
                            "Locate…",
                            None,
                            false,
                            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.locate_missing_selected(cx);
                            }),
                        )
                    }))
                    // 12. Reveal in Finder
                    .child({
                        let path_for_reveal = path.clone();
                        Self::render_menu_item(
                            &hover,
                            "reveal-menu-btn",
                            "Reveal in Finder",
                            Some("⇧⌘R"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.reveal_item_in_finder(&path_for_reveal, cx);
                            }),
                        )
                    })
                    // 13. Open in default application
                    .child({
                        let path_for_open = path.clone();
                        Self::render_menu_item(
                            &hover,
                            "open-with-menu-btn",
                            "Open in Default App",
                            None,
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_item_in_default_app(&path_for_open, cx);
                            }),
                        )
                    })
                    // 14. Edit in the external editor configured in Preferences
                    .child({
                        let path_for_edit = path.clone();
                        Self::render_menu_item(
                            &hover,
                            "edit-external-menu-btn",
                            "Edit in External Editor",
                            Some("⌘E"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.open_item_in_external_editor(&path_for_edit, cx);
                            }),
                        )
                    })
                    // 15. Edit a copy next to the original (no Lumina edits)
                    .child({
                        let item_for_copy = item.clone();
                        Self::render_menu_item(
                            &hover,
                            "edit-copy-external-menu-btn",
                            "Edit a Copy in External Editor",
                            Some("⌥⌘E"),
                            false,
                            cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.edit_item_externally(item_for_copy.clone(), true, cx);
                            }),
                        )
                    })
                    // Quick-edit rotation is a display-only catalog edit. Videos
                    // are omitted rather than offering a control that cannot
                    // rotate the player surface.
                    .children((!is_video && !is_trash).then(|| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(Self::render_menu_item(
                                &hover,
                                "quick-edit-rotate-left",
                                "Rotate Left",
                                Some("⇧R"),
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_context_menu(cx);
                                    this.rotate_selection(ViewerRotation::Left, cx);
                                }),
                            ))
                            .child(Self::render_menu_item(
                                &hover,
                                "quick-edit-rotate-right",
                                "Rotate Right",
                                Some("R"),
                                false,
                                cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.close_context_menu(cx);
                                    this.rotate_selection(ViewerRotation::Right, cx);
                                }),
                            ))
                    }))
                    // Divider
                    .child(Self::render_divider())
                    // 15. Delete / Trash
                    .child(if is_trash {
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(Self::render_menu_item(
                                &hover,
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
                                &hover,
                                "delete-perm-menu-btn",
                                "Delete Immediately…",
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
                            &hover,
                            "trash-menu-btn",
                            "Delete…",
                            Some("⌘⌫"),
                            true,
                            cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.close_context_menu(cx);
                                this.prompt_delete_selected(cx);
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
        hover: &RowHover,
        id_str: &'static str,
        label: impl Into<SharedString>,
        shortcut: Option<&'static str>,
        is_destructive: bool,
        on_click_handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let trailing = if let Some(sc) = shortcut {
            div()
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(if hover.hovered == Some(id_str) && !is_destructive {
                    theme::accent_hint()
                } else {
                    theme::text_subtle()
                })
                .child(sc)
                .into_any_element()
        } else {
            Empty.into_any_element()
        };
        Self::render_menu_row(
            hover,
            id_str,
            label,
            trailing,
            is_destructive,
            on_click_handler,
        )
    }

    fn render_menu_row(
        hover: &RowHover,
        id_str: &'static str,
        label: impl Into<SharedString>,
        trailing: AnyElement,
        is_destructive: bool,
        on_click_handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let is_hovered = hover.hovered == Some(id_str);
        let app = hover.app.clone();
        let label: SharedString = label.into();
        div()
            .id(id_str)
            .debug_selector(move || id_str.to_string())
            .role(Role::MenuItem)
            .aria_label(label.clone())
            .px_2p5()
            .h(px(26.0))
            .rounded(theme::RADIUS_SM)
            .cursor_pointer()
            .text_color(if is_destructive {
                theme::danger_text()
            } else if is_hovered {
                theme::accent_fg()
            } else {
                theme::text()
            })
            .hover(move |s| {
                if is_destructive {
                    s.bg(theme::danger_bg())
                } else {
                    s.bg(theme::accent())
                }
            })
            .on_hover(move |entered, _w, cx| {
                app.update(cx, |this, cx| {
                    if let Some(menu) = this.context_menu_state.as_mut() {
                        if *entered {
                            menu.hovered = Some(id_str);
                        } else if menu.hovered == Some(id_str) {
                            menu.hovered = None;
                        }
                        cx.notify();
                    }
                })
                .ok();
            })
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(div().text_size(theme::TEXT_BODY).child(label))
            .child(trailing)
            .on_click(on_click_handler)
    }
}
