use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::*;

use crate::app::LuminaApp;
use crate::motion;
use crate::theme;
use crate::views::inspector::InspectorView;
use crate::views::onboarding::OnboardingView;
use crate::views::sidebar::{AlbumSelection, SidebarView};
use crate::views::viewer_state::ViewerRotation;

pub(crate) mod bucket_cards;
mod card;
pub(crate) mod empty_state;
pub(super) mod error_banner;
mod grid;
pub(crate) mod mosaic;
pub(crate) mod overlays;
pub(crate) mod rail;
mod shortcuts;
mod status_bar;
mod toolbar;
mod video_caption;
#[cfg(test)]
pub(crate) use toolbar::FORCE_CLIENT_DECORATIONS;

/// Menu actions every root handles: the welcome screen, "Building your
/// library" and the library window. Only actions that need no overlay go
/// here; one whose overlay a root does not paint must not be registered there.
fn app_actions(div: Div, cx: &mut Context<LuminaApp>) -> Div {
    div.on_action(cx.listener(|this, _: &QuitApp, _w, cx| {
        this.request_quit(cx);
    }))
    .on_action(cx.listener(|this, _: &OpenLibrary, _w, cx| {
        // The menu bar is reachable over a full-screen slideshow; the
        // picker would open behind it while the show plays on.
        if this.slideshow.is_some() {
            return;
        }
        this.pick_folder_dialog(cx);
    }))
}

impl Render for LuminaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = self.focus_handle.clone();
        self.sync_field_input(window, &root, false, cx);
        // Evict replaced viewer frames before painting the new one.
        crate::app::viewer_actions::evict_retired_images(
            std::mem::take(&mut self.retired_images),
            window,
            cx,
        );
        self.observe_window_bounds(window, cx);

        // Welcome (a header without actions) and first scan have no header
        // popovers: drop them so their keys don't shadow these screens' and
        // they don't return later.
        if self.grid_state.is_none() || self.first_scan.is_some() {
            self.close_header_popovers();
        }
        if self.grid_state.is_none() {
            self.check_recent_availability(cx);
            // Preferences and About paint here (Review Focus 7: never
            // register an action whose overlay this root does not paint).
            return self.render_welcome(cx);
        }

        if let Some(scan) = self.first_scan.clone() {
            return app_actions(div(), cx)
                .size_full()
                .font_family(theme::FONT_SANS)
                .track_focus(&self.focus_handle)
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.handle_key_down(event, window, cx);
                }))
                .flex()
                .flex_col()
                .relative()
                .child(self.render_error_banner(cx))
                .child(crate::views::first_scan::render(self, &scan, cx))
                .child(self.render_toast_overlay(cx))
                .into_any_element();
        }

        self.refresh_active_view(cx);
        self.refresh_top_matches(cx);
        self.refresh_map_points(cx);
        self.prepare_albums_overview(window, cx);
        self.prepare_album_page(cx);
        self.close_header_popovers_under_modals();
        // A mouse-up outside an inactive window never reaches the root.
        if !window.is_window_active() {
            self.query_bar.held_for_click = false;
        }
        let total_count = self.displayed_count();

        let view_label = self.view_label();

        // The inspector stays while Info is on; with no selection it shows an
        // empty state rather than vanishing under a lit toggle.
        let is_fullscreen_viewer = self.is_fullscreen_viewer;
        // The Albums overview, the Tags page and the Downloads screen hold
        // no media: the panel would have nothing to show, so it stays hidden
        // there (Info stays on for other views). Review Duplicates hides it
        // too: the compare cards show each copy's facts.
        let inspector_shown = self.inspector_open
            && !is_fullscreen_viewer
            && !self.non_media_page_shown()
            && !self.duplicates_review_shown();

        let app_root = app_actions(
            crate::app::text_input::field_input_actions(div(), &cx.entity()),
            cx,
        );
        let root = app_root
            .id("lumina-app-root")
            .debug_selector(|| "app-root".to_string())
            .font_family(theme::FONT_SANS)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }));
        let root = self
            .library_actions(root, cx)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    // Any click outside the search field should return keyboard
                    // shortcuts to the app. The field's own click re-focuses it.
                    // The Search screen waits for the mouse-up to go, so the
                    // header does not shrink under the click.
                    if this.search_focused && this.search_screen_shown() {
                        this.query_bar.held_for_click = true;
                    }
                    this.blur_search(cx);
                }),
            )
            // Capture: runs whatever the click hits, which may stop it.
            .capture_any_mouse_up(cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                if std::mem::take(&mut this.query_bar.held_for_click) {
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, window, cx| {
                if this.is_dragging_sidebar_resizer {
                    let x: f32 = e.position.x.into();
                    this.sidebar_width = x.clamp(180.0, 320.0);
                    cx.notify();
                } else if this.is_dragging_inspector_resizer {
                    let x: f32 = e.position.x.into();
                    let win_w: f32 = window.viewport_size().width.into();
                    this.inspector_width = (win_w - x).clamp(260.0, 380.0);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseUpEvent, _w, cx| {
                    if this.is_dragging_sidebar_resizer || this.is_dragging_inspector_resizer {
                        this.is_dragging_sidebar_resizer = false;
                        this.is_dragging_inspector_resizer = false;
                        this.save_config();
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .bg(theme::bg_0())
            .flex()
            .flex_col()
            .relative()
            // Sidebar (full height) + resizer + main column (banner, header, canvas, inspector)
            .child(self.render_library_columns(
                &view_label,
                total_count,
                inspector_shown,
                window,
                cx,
            ));
        self.render_overlays(root, window, cx).into_any_element()
    }
}

impl LuminaApp {
    /// Saves the window bounds as the user moves or resizes the window,
    /// throttled while a live resize runs. Subscribes once.
    fn observe_window_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_bounds_subscription.is_some() {
            return;
        }
        self.window_bounds_subscription =
            Some(cx.observe_window_bounds(window, |this, window, cx| {
                let bounds = window.bounds();
                this.window_bounds = Some((
                    bounds.origin.x.into(),
                    bounds.origin.y.into(),
                    bounds.size.width.into(),
                    bounds.size.height.into(),
                ));
                // Throttle disk writes while the user is live-resizing.
                let now = std::time::Instant::now();
                let should_save = this.last_bounds_save.is_none_or(|last| {
                    now.duration_since(last) >= std::time::Duration::from_millis(750)
                });
                if should_save {
                    this.last_bounds_save = Some(now);
                    this.save_config();
                }
                cx.notify();
            }));
    }

    /// The welcome root, before any library is open: the dormant shell
    /// around the welcome, with settings and the toast above it.
    fn render_welcome(&mut self, cx: &mut Context<Self>) -> AnyElement {
        app_actions(div(), cx)
            // Like ⌘,: Settings… while settings is open closes it.
            .on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                if this.preferences_open {
                    this.close_preferences(cx);
                } else {
                    this.open_preferences(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                this.open_about(cx);
            }))
            .on_action(cx.listener(|this, _: &ImportMedia, _w, cx| {
                this.pick_new_library_for_import(cx);
            }))
            .size_full()
            .font_family(theme::FONT_SANS)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .relative()
            .bg(theme::bg_0())
            // The app shell with a dormant sidebar and the
            // All Media header, around the welcome.
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(crate::views::sidebar::dormant::render_dormant(self))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            // Settings draws the banner while it covers this.
                            .when(!self.preferences_open, |col| {
                                col.child(self.render_error_banner(cx))
                            })
                            .child(crate::views::page_header::page_header(
                                Empty,
                                crate::i18n::t("view.all_media"),
                                Some(crate::i18n::t("header.no_media").into()),
                                Empty,
                                false,
                            ))
                            .child(
                                div()
                                    .id("welcome-body")
                                    .debug_selector(|| "welcome-body".to_string())
                                    .flex_1()
                                    .min_h_0()
                                    // A small window scrolls the welcome.
                                    .overflow_y_scroll()
                                    .track_scroll(&self.welcome.body_scroll)
                                    .child(OnboardingView::render(self.welcome_props(cx))),
                            ),
                    ),
            )
            .child(motion::overlay_in(
                self.preferences_open,
                crate::views::preferences::render_preferences_modal(self, cx),
                "preferences-in",
            ))
            // Above settings, which is opaque.
            .child(self.render_toast_overlay(cx))
            .into_any_element()
    }

    /// The library root's menu actions, each registered only where it can
    /// act on what the user sees.
    fn library_actions(&self, root: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        let editing = self.edits.session.is_some();
        // Compare shows (not the viewer opened from it), or a slideshow
        // plays (the menu bar stays reachable in full screen): menu items
        // that act on the hidden selection, open a dialog or move the view
        // are disabled.
        let slideshow = self.slideshow.is_some();
        let presenting = (self.compare.is_some() && !self.is_fullscreen_viewer) || slideshow;
        // The viewer opened from compare, too: these would play, move or
        // trash the view behind compare (the palette leaves them out).
        let holds_view = self.compare_holds_view();
        // Settings is an opaque full-window layer: items that open a dialog
        // under it or act on the hidden library are disabled.
        let settings = self.preferences_open;
        let open_ok = !editing && !presenting && !settings;
        root
            // Compare leaves these on; under a slideshow they would change
            // the panels or open the sheet unseen behind the slide.
            .when(!slideshow, |root| {
                root.on_action(cx.listener(|this, _: &ShowShortcuts, _w, cx| {
                    this.toggle_shortcuts_overlay(cx);
                }))
            })
            // Under settings a toggle would flip and save a setting the user
            // cannot see.
            .when(!slideshow && !settings, |root| {
                root.on_action(cx.listener(|this, _: &ToggleSidebar, _w, cx| {
                    this.toggle_sidebar(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleInspector, _w, cx| {
                    this.toggle_inspector(cx);
                }))
                .when(self.album_panel_fits(), |root| {
                    root.on_action(cx.listener(|this, _: &ToggleAlbumPanel, _w, cx| {
                        this.toggle_album_panel(cx);
                    }))
                })
            })
            // Edit mode: no handler means a disabled menu item. These open
            // overlays that take the keys, change media, or leave the photo
            // under the draft. Rotate and Revert go to the draft; Quit and
            // Open Library stay (they discard the draft and say so).
            .when(!editing && !presenting, |root| {
                // Like ⌘,: Settings… while settings is open closes it.
                root.on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                    if this.preferences_open {
                        this.close_preferences(cx);
                    } else {
                        this.open_preferences(cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                    this.open_about(cx);
                }))
                // Settings › Library offers Rescan too, so it stays on there.
                .on_action(cx.listener(|this, _: &RescanLibrary, _w, cx| {
                    this.rescan_library(cx);
                }))
                .on_action(cx.listener(
                    |this, _: &ShowCommandPalette, _w, cx| {
                        this.open_command_palette(cx);
                    },
                ))
            })
            .when(open_ok, |root| {
                root.on_action(cx.listener(|this, _: &ImportMedia, _w, cx| {
                    this.pick_import_folders(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportNetworkVideo, _w, cx| {
                    this.open_network_import_dialog(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleDownloads, _w, cx| {
                    this.toggle_downloads_panel(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowAllDownloads, _w, cx| {
                    this.show_downloads_screen(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportXmp, _w, cx| {
                    this.import_sidecars_selected(cx);
                }))
                .on_action(cx.listener(|this, _: &ManageTemplates, _w, cx| {
                    this.open_template_editor(cx);
                }))
                .on_action(cx.listener(|this, _: &NewAlbum, _w, cx| {
                    this.new_album(cx);
                }))
                .on_action(cx.listener(|this, _: &AddToAlbum, _w, cx| {
                    this.open_album_picker(cx);
                }))
                .on_action(cx.listener(|this, _: &NewSmartAlbum, _w, cx| {
                    this.open_album_builder(cx);
                }))
                .on_action(cx.listener(|this, _: &ExportOriginals, _w, cx| {
                    this.export_pick_folder(lumina_core::ExportPreset::Originals, cx);
                }))
                .on_action(cx.listener(|this, _: &ExportAsJpeg, _w, cx| {
                    this.export_pick_folder(lumina_core::ExportPreset::Jpeg2048, cx);
                }))
                // File > Share… exists on macOS only; elsewhere nothing sends it.
                .on_action(cx.listener(|this, _: &ShareSelection, _w, cx| {
                    #[cfg(target_os = "macos")]
                    this.share_selection(None, cx);
                    #[cfg(not(target_os = "macos"))]
                    let _ = (this, cx);
                }))
                .on_action(cx.listener(|this, _: &Compare, _w, cx| {
                    this.open_compare(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportSmartAlbumRules, _w, cx| {
                    this.import_smart_album_rules(cx);
                }))
                // The editor opens the file without the draft's edits.
                .on_action(cx.listener(|this, _: &EditInExternalEditor, _w, cx| {
                    this.edit_selected_externally(false, cx);
                }))
                .on_action(cx.listener(
                    |this, _: &EditCopyInExternalEditor, _w, cx| {
                        this.edit_selected_externally(true, cx);
                    },
                ))
            })
            .when(open_ok && !holds_view, |root| {
                root.on_action(cx.listener(|this, _: &DeleteRejected, _w, cx| {
                    this.delete_rejected(cx);
                }))
                .on_action(cx.listener(|this, _: &Slideshow, window, cx| {
                    this.start_slideshow(window, cx);
                }))
                .on_action(cx.listener(|this, _: &ShowMap, _w, cx| {
                    this.show_map(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowAlbums, _w, cx| {
                    this.show_albums_overview(cx);
                }))
                .on_action(cx.listener(|this, _: &GoBack, _w, cx| {
                    this.navigate_back(cx);
                }))
                .on_action(cx.listener(|this, _: &GoForward, _w, cx| {
                    this.navigate_forward(cx);
                }))
            })
            .when(!presenting && !settings, |root| {
                root.on_action(cx.listener(|this, _: &RotateLeft, _w, cx| {
                    this.rotate_selection(ViewerRotation::Left, cx);
                }))
                .on_action(cx.listener(|this, _: &RotateRight, _w, cx| {
                    this.rotate_selection(ViewerRotation::Right, cx);
                }))
                .on_action(cx.listener(|this, _: &EditPhoto, _w, cx| {
                    this.enter_edit_mode(cx);
                }))
                .on_action(cx.listener(|this, _: &RevertToOriginal, _w, cx| {
                    this.revert_selection_to_original(cx);
                }))
            })
    }

    /// The sidebar, its resizer and the main column: banner, header, the
    /// canvas with its bars, and the inspector or album panel.
    fn render_library_columns(
        &mut self,
        view_label: &str,
        total_count: usize,
        inspector_shown: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_1()
            .size_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            // Sidebar
            .child(SidebarView::render(self, cx))
            // Sidebar Resizer Drag Handle (180–320px)
            .child(if !self.sidebar_collapsed {
                div()
                    .id("sidebar-resizer")
                    .w(px(4.0))
                    .h_full()
                    .cursor_col_resize()
                    .hover(|s| s.bg(theme::line_2()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            this.is_dragging_sidebar_resizer = true;
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            } else {
                Empty.into_any_element()
            })
            // Main column: the banner and header sit beside the
            // full-height sidebar, above the canvas and inspector.
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    // Settings draws the banner while it covers this.
                    .when(!self.preferences_open, |col| {
                        col.child(self.render_error_banner(cx))
                    })
                    .child(if self.search_screen_shown() {
                        self.render_search_header(cx).into_any_element()
                    } else if let Some(id) = self.album_page_album() {
                        crate::views::album_page::header::render(self, id, window, cx)
                            .into_any_element()
                    } else {
                        self.render_toolbar(view_label, window, cx)
                            .into_any_element()
                    })
                    .child(crate::views::album_page::selection_bar::render(
                        self, window, cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_row()
                            .overflow_hidden()
                            // Center Canvas: the browser grid is the only non-modal
                            // content. The media viewer is painted as a fullscreen
                            // overlay above it (see render_viewer_modal).
                            // The filter and status bars belong to this column; the
                            // inspector sits beside it, under the header.
                            // The Albums overview and the Tags page
                            // replace the whole column: no grid,
                            // status bar or dock.
                            .child(if self.tags_page_shown() {
                                div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(crate::views::tag_manager::render(self, window, cx))
                                    .when(!self.preferences_open, |d| {
                                        d.child(self.render_toast_overlay(cx))
                                    })
                                    .into_any_element()
                            } else if self.albums_overview_shown() {
                                div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(crate::views::albums_overview::render(self, cx))
                                    .when(!self.preferences_open, |d| {
                                        d.child(self.render_toast_overlay(cx))
                                    })
                                    .into_any_element()
                            } else if self.downloads_screen_shown() {
                                div()
                                    .relative()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(crate::views::downloads_screen::render(self, window, cx))
                                    .when(!self.preferences_open, |d| {
                                        d.child(self.render_toast_overlay(cx))
                                    })
                                    .into_any_element()
                            } else {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .flex()
                                    .flex_col()
                                    .children(self.render_trash_note())
                                    .children(self.render_filter_bar(cx))
                                    .children(self.render_top_matches(window, cx))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_1()
                                            .min_h_0()
                                            .flex()
                                            .flex_row()
                                            .child(
                                                if self.selection == AlbumSelection::Duplicates {
                                                    crate::views::stacks::render_duplicates(
                                                        self,
                                                        total_count,
                                                        cx,
                                                    )
                                                } else if self.map_shown() {
                                                    crate::views::map::render(self, cx)
                                                } else if self.mosaic_shown() {
                                                    self.render_mosaic(total_count, cx)
                                                } else {
                                                    self.render_browser_grid(total_count, cx)
                                                        .into_any_element()
                                                },
                                            )
                                            // Pinned Vertical Year Rail
                                            .child(self.render_year_rail(cx))
                                            // Zoom pill, selection dock and toast, over the grid
                                            // column only, stacked bottom-up: dock, pill, toast.
                                            .child(self.render_zoom_pill(cx))
                                            .child(self.render_action_dock(cx))
                                            .when(!self.preferences_open, |d| {
                                                d.child(self.render_toast_overlay(cx))
                                            }),
                                    )
                                    .child(self.render_status_bar(total_count, cx))
                                    .into_any_element()
                            })
                            // Inspector Resizer Drag Handle (260–380px)
                            .child(if inspector_shown {
                                div()
                                    .id("inspector-resizer")
                                    .w(px(4.0))
                                    .h_full()
                                    .cursor_col_resize()
                                    .hover(|s| s.bg(theme::line_2()))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                                            this.is_dragging_inspector_resizer = true;
                                            cx.notify();
                                        }),
                                    )
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            })
                            // Right Slide-out Metadata & Location Inspector
                            .child(if inspector_shown {
                                div()
                                    .debug_selector(|| "inspector-panel".to_string())
                                    .flex_none()
                                    .w(px(self.inspector_width))
                                    .h_full()
                                    .border_l_1()
                                    .border_color(theme::line())
                                    .child(InspectorView::render(self, cx))
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            })
                            // With Info off an open album's settings
                            // panel takes the right column.
                            .child(crate::views::album_page::panel::render(self, cx)),
                    ),
            )
    }

    /// The layers above the library columns, bottom to top: compare, the
    /// viewer, modals and menus, the shortcut sheet, settings, the palette
    /// and the drop target.
    fn render_overlays(
        &mut self,
        root: Stateful<Div>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        root
            // 5b. Compare / survey, under the viewer it can open
            .child(crate::views::compare::render(self, cx))
            // 6. Fullscreen Detail Viewer Modal
            .child(self.render_viewer_modal(cx))
            // 7. Visual Smart Album Builder Modal
            .child(motion::overlay_in(
                self.album_draft.modal_open,
                self.render_album_builder_modal(cx),
                "album-builder-in",
            ))
            // 8. Delete / Empty Trash Confirmation Modal
            .child(motion::overlay_in(
                self.delete_confirm_state.is_some(),
                self.render_delete_confirm_modal(cx),
                "delete-confirm-in",
            ))
            // 8b. Smart album delete confirmation
            .child(motion::overlay_in(
                self.album_draft.confirm_delete.is_some(),
                crate::views::smart_album_delete::render(self, cx),
                "smart-album-delete-in",
            ))
            // 8b2. Album delete confirmation
            .child(motion::overlay_in(
                self.albums.confirm_delete.is_some(),
                crate::views::album_delete::render(self, cx),
                "album-delete-in",
            ))
            // 8b3. Add to Album picker
            .child(motion::overlay_in(
                self.albums.picker.is_some(),
                crate::views::album_picker::render(self, cx),
                "album-picker-in",
            ))
            // 8c. Import dialog
            .child(motion::overlay_in(
                self.import.dialog.is_some(),
                crate::views::import_dialog::render(self, cx),
                "import-dialog-in",
            ))
            // 8d. Network video import dialog
            .child(motion::overlay_in(
                self.network_import.stage.is_some(),
                crate::views::network_import_dialog::render(self, cx),
                "network-import-dialog-in",
            ))
            // 8d2. Metadata template editor
            .child(motion::overlay_in(
                self.templates.editor.is_some(),
                crate::views::template_editor::render(self, cx),
                "template-editor-in",
            ))
            // 8e. Downloads dropdown
            .child(motion::overlay_in(
                self.downloads.open,
                crate::views::downloads_panel::render(self, cx),
                "downloads-panel-in",
            ))
            // 8f. "Downloads are still running" before quit / library switch
            .child(motion::overlay_in(
                self.downloads.confirm.is_some(),
                crate::views::downloads_confirm::render(self, cx),
                "downloads-confirm-in",
            ))
            // 9. Media Card Context Menu Overlay
            .child(motion::overlay_in(
                self.context_menu_state.is_some(),
                self.render_context_menu_overlay(window, cx),
                "context-menu-in",
            ))
            // 9b. Sort popover
            .child(motion::overlay_in(
                self.sort_menu_open,
                crate::views::sort_menu::render_sort_menu_overlay(self, cx),
                "sort-menu-in",
            ))
            // 9b2. Album page More menu backdrop (the card hangs from More)
            .child(motion::overlay_in(
                self.albums.page.menu.is_some(),
                crate::views::album_page::render_menu_backdrop(self, cx),
                "album-menu-in",
            ))
            // 9c. Add filter menu backdrop (the card hangs from its opener)
            .child(motion::overlay_in(
                self.filter_menu.is_some(),
                crate::views::filter_bar::render_filter_menu_overlay(self, cx),
                "filter-menu-in",
            ))
            // 11. Keyboard Shortcuts Cheat Sheet
            .child(motion::overlay_in(
                self.shortcuts_overlay_open,
                self.render_shortcuts_overlay(cx),
                "shortcuts-in",
            ))
            // 12. Settings (About is its last category): a full-window page
            .child(motion::overlay_in(
                self.preferences_open,
                crate::views::preferences::render_preferences_modal(self, cx),
                "preferences-in",
            ))
            // 12b. The toast over settings (opaque); the columns draw it
            // otherwise.
            .when(self.preferences_open, |root| {
                root.child(self.render_toast_overlay(cx))
            })
            // 13. Command palette (topmost)
            .child(motion::overlay_in(
                self.command_palette_open,
                crate::views::command_palette::render_command_palette(self, cx),
                "command-palette-in",
            ))
            // 14. Files dragged in from Finder (shown only during a drag)
            .children(
                (self.grid_state.is_some() && !self.import_blocked_by_modal())
                    .then(|| crate::views::import_dialog::render_drop_target(cx)),
            )
    }
}
