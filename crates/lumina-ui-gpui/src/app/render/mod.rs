use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::motion;
use crate::theme;
use crate::views::inspector::InspectorView;
use crate::views::onboarding::OnboardingView;
use crate::views::sidebar::{AlbumSelection, SidebarView};
use crate::views::viewer_state::ViewerRotation;

mod grid;
mod overlays;
mod rail;
mod status_bar;
mod toolbar;
#[cfg(test)]
pub(crate) use toolbar::FORCE_CLIENT_DECORATIONS;

impl LuminaApp {
    /// Banner for recoverable failures: alert icon, title and description,
    /// optional Retry, and a bordered dismiss button.
    pub(crate) fn render_error_banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.error_message.clone() else {
            return Empty.into_any_element();
        };
        let (title, detail) = match message.split_once(": ") {
            Some((title, detail)) => (title.to_string(), Some(detail.to_string())),
            None => (message, None),
        };
        div()
            .id("app-error-banner")
            .debug_selector(|| "error-banner".to_string())
            .w_full()
            .px_4()
            .py_2()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .bg(theme::danger_bg())
            .border_b_1()
            .border_color(theme::danger_border())
            .child(render_icon(
                Icon::TriangleAlert,
                px(16.0),
                theme::danger_text(),
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .debug_selector(|| "error-banner-title".to_string())
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child(title),
                    )
                    .children(detail.map(|detail| {
                        div()
                            .debug_selector(|| "error-banner-detail".to_string())
                            .text_xs()
                            .text_color(theme::text_muted())
                            .child(detail)
                    })),
            )
            .children(self.error_retry.is_some().then(|| {
                theme::control("app-error-banner-retry")
                    .debug_selector(|| "error-banner-retry".to_string())
                    .tooltip(theme::tooltip_label("Retry (⌘K Retry Failed Action)"))
                    .child("Retry")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.retry_error(cx);
                    }))
            }))
            .child(
                div()
                    .id("app-error-banner-dismiss")
                    .debug_selector(|| "error-banner-dismiss".to_string())
                    .size(px(26.0))
                    .flex_none()
                    .rounded(theme::RADIUS_MD)
                    .border_1()
                    .border_color(theme::danger_border())
                    .hover(|s| s.bg(theme::danger_bg_hover()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(theme::tooltip_label("Dismiss"))
                    .child(render_icon(Icon::X, px(12.0), theme::text()))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.dismiss_error(cx);
                    })),
            )
            .into_any_element()
    }
}

/// The shortcut sheet: titled sections of (keys, what they do). Keep in
/// step with `app/keymap`.
const SHORTCUT_SECTIONS: [(&str, &[(&str, &str)]); 10] = [
    (
        "GENERAL",
        &[
            ("⌘K", "Command palette"),
            ("⌘F", "Focus search"),
            ("⌘O", "Open library…"),
            ("⇧⌘I", "Import…"),
            ("⌘,", "Preferences"),
            ("⌘R", "Rescan library"),
            ("⌘Z / ⇧⌘Z", "Undo / redo"),
            ("⌘[ / ⌘]", "Back / Forward"),
            ("⌘N", "New album"),
            ("⌥⌘N", "New smart album"),
            ("⌥⌘L", "Show or hide downloads"),
            ("⌃⌘S", "Show or hide sidebar"),
            ("I", "Toggle inspector"),
            ("⌘I", "Get Info window"),
            ("F", "Favorite"),
            ("⇧⌘R", "Show in Finder"),
            ("⌥⌘C", "Copy file path"),
            ("⌘E", "Edit in external editor"),
            ("⌥⌘E", "Edit a copy in external editor"),
            ("Esc", "Close / go back"),
            ("⌘/", "Toggle this help"),
        ],
    ),
    (
        "BROWSING",
        &[
            ("← ↑ → ↓", "Move selection"),
            ("⇧ + arrows", "Extend selection"),
            ("⌘A", "Select all"),
            ("⇧⌘A", "Add to album…"),
            ("⌫ in an album", "Remove from album"),
            ("Home / End", "First / last item"),
            ("PageUp / PageDown", "Jump four rows"),
            ("⌘+ / ⌘-", "Grid density"),
            ("Space / Enter", "Open in viewer"),
            ("T", "Tag selection"),
            ("R / ⇧R", "Rotate right / left"),
            ("P / X / U", "Pick / reject / remove flag"),
            ("1–5 / 0", "Rate / clear rating"),
            ("⌫", "Delete…"),
            ("C", "Compare the selection (2–9)"),
        ],
    ),
    (
        "COMPARE",
        &[
            ("← / → / Tab", "Focus another pane"),
            ("P / X / U, 0–5", "Flag / rate the focused pane"),
            ("⌫", "Remove the pane (keeps the file)"),
            ("Enter", "Open in viewer (Esc returns)"),
            ("+ / - / Z / ⌘0–⌘4", "Zoom every pane"),
            ("Esc", "Close compare"),
        ],
    ),
    (
        "SIMILAR SHOTS (DUPLICATES)",
        &[
            ("↑ / ↓", "Previous / next stack"),
            ("Enter on a stack", "Keep picks, reject the rest"),
            ("⇧Enter", "Reject all but the first"),
            (
                "⌘K Show Similar Shots",
                "This tab (⌘K Show Exact Copies: the other)",
            ),
        ],
    ),
    (
        "VIEWER",
        &[
            ("[ / ]", "Previous / next"),
            ("← / →", "Previous / next photo"),
            ("Enter / Esc", "Close viewer"),
            ("⌃⌘F", "Full screen"),
            ("H", "Hide / show controls"),
            ("+ / -", "Zoom in / out"),
            ("Pinch / ⌘ scroll", "Zoom at pointer"),
            ("Scroll / drag", "Pan when zoomed"),
            ("⌘0 / ⌘1 / ⌘2 / ⌘4", "Fit / 100% / 200% / 400%"),
            ("⌘3", "Fill"),
            ("Z", "Toggle 1:1 zoom"),
            ("O", "Open in default app"),
            ("P / X / U", "Pick / reject / remove flag, then next"),
            ("1–5 / 0", "Rate / clear rating, then next"),
            ("R / ⇧R", "Rotate right / left"),
            ("E", "Edit photo"),
        ],
    ),
    (
        "SLIDESHOW",
        &[
            ("⌥⌘S", "Play the view from the selection"),
            ("Space", "Pause / resume"),
            ("← / →", "Previous / next slide"),
            ("Esc", "End the slideshow"),
        ],
    ),
    (
        "EDITING A PHOTO",
        &[
            ("R / ⇧R", "Rotate right / left"),
            ("- / =", "Darker / brighter (⅓ stop)"),
            ("A", "Auto enhance on / off"),
            ("C", "Crop; again: next aspect"),
            ("⇧C", "Crop on / off (keeps the crop)"),
            ("← → ↑ ↓", "Move the crop 1 % (⇧ 10 %)"),
            ("⌥ arrows", "Resize the crop"),
            ("Enter", "Done (⌘Z undoes)"),
            ("Esc", "Cancel (while cropping: leave crop)"),
        ],
    ),
    (
        "VIDEO",
        &[
            ("Space / K", "Play / pause"),
            ("J / ←", "Back 5 s"),
            ("→", "Forward 5 s"),
            (", / .", "Previous / next frame"),
            ("L", "Loop"),
            ("M", "Mute"),
            ("⌥P", "Picture-in-Picture on / off"),
        ],
    ),
    (
        "IN THE TAG MANAGER",
        &[
            ("⇧⌘A", "Add an alias for the keyword"),
            ("⇧⌘M", "Move the keyword to another parent"),
        ],
    ),
    (
        "IN METADATA TEMPLATES",
        &[
            ("Tab / ⇧Tab", "Next / previous field"),
            ("↑ / ↓", "Previous / next template"),
            ("Enter", "Save the template"),
            ("⇧⌘⌫", "Delete the template (⌘Z undoes)"),
            ("Esc", "Close without saving"),
        ],
    ),
];

impl LuminaApp {
    /// Keyboard shortcut cheat sheet (Cmd+/), also serving as keyboard discoverability.
    fn render_shortcuts_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.shortcuts_overlay_open {
            return Empty.into_any_element();
        }
        div()
            .id("shortcuts-overlay")
            .debug_selector(|| "shortcuts-overlay".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::modal_dim())
            .flex()
            .justify_center()
            .items_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_shortcuts_overlay(cx);
                }),
            )
            .child(
                div()
                    .id("shortcuts-card")
                    .debug_selector(|| "shortcuts-card".to_string())
                    .w(px(520.0))
                    .max_h(px(560.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.shortcuts_scroll)
                    .bg(theme::bg_pane())
                    .border_1()
                    .border_color(theme::border())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::text())
                                    .child("Keyboard Shortcuts"),
                            )
                            .child(
                                div()
                                    .id("shortcuts-close")
                                    .debug_selector(|| "shortcuts-close".to_string())
                                    .cursor_pointer()
                                    .child(render_icon(Icon::X, px(12.0), theme::text()))
                                    .tooltip(theme::tooltip_label("Close"))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.close_shortcuts_overlay(cx);
                                    })),
                            ),
                    )
                    .children(SHORTCUT_SECTIONS.into_iter().map(|(title, rows)| {
                        div()
                            .pt_2()
                            .flex()
                            .flex_col()
                            .child(theme::section_label(title))
                            .children(rows.iter().map(|&(keys, description)| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .py_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::text())
                                            .child(description),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(theme::FONT_MONO)
                                            .text_color(theme::text_muted())
                                            .child(keys),
                                    )
                            }))
                    })),
            )
            .into_any_element()
    }
}

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
        if self.window_bounds_subscription.is_none() {
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

        if self.grid_state.is_none() {
            self.check_recent_availability(cx);
            // Preferences and About paint here (Review Focus 7: never
            // register an action whose overlay this root does not paint).
            return app_actions(div(), cx)
                .on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                    this.open_preferences(cx);
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
                .child(self.render_error_banner(cx))
                .child(OnboardingView::render(self.welcome_props(cx)))
                .child(self.render_toast_overlay(cx))
                .child(motion::overlay_in(
                    self.preferences_open,
                    crate::views::preferences::render_preferences_modal(self, cx),
                    "preferences-in",
                ))
                .into_any_element();
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
        let total_count = self.displayed_count();

        let view_label: String = match &self.selection {
            AlbumSelection::AllLibrary => "All Photos & Videos".into(),
            AlbumSelection::System(album) => album.title().into(),
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|a| a.id == *id)
                .map(|a| a.title.clone())
                .unwrap_or_else(|| "Album".into()),
            AlbumSelection::Album(id) => self
                .album_title(*id)
                .map_or_else(|| t("album.fallback_title"), str::to_string),
            AlbumSelection::Tag(tag) => format!("#{tag}"),
            AlbumSelection::Untagged => "Untagged".into(),
            AlbumSelection::Duplicates => "Duplicates".into(),
            AlbumSelection::Trash => "Recently Deleted".into(),
        };

        // The inspector stays while Info is on; with no selection it shows an
        // empty state rather than vanishing under a lit toggle.
        let inspector_open = self.inspector_open;
        let is_fullscreen_viewer = self.is_fullscreen_viewer;

        let editing = self.edits.session.is_some();
        // Compare shows (not the viewer opened from it), or a slideshow
        // plays (the menu bar stays reachable in full screen): menu items
        // that act on the hidden selection, open a dialog or move the view
        // are disabled.
        let slideshow = self.slideshow.is_some();
        let presenting = (self.compare.is_some() && !is_fullscreen_viewer) || slideshow;
        let app_root = app_actions(
            crate::app::text_input::field_input_actions(div(), &cx.entity()),
            cx,
        );
        app_root
            .id("lumina-app-root")
            .debug_selector(|| "app-root".to_string())
            .font_family(theme::FONT_SANS)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            // Compare leaves these on; under a slideshow they would change
            // the panels or open the sheet unseen behind the slide.
            .when(!slideshow, |root| {
                root.on_action(cx.listener(|this, _: &ShowShortcuts, _w, cx| {
                    this.toggle_shortcuts_overlay(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleSidebar, _w, cx| {
                    this.toggle_sidebar(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleInspector, _w, cx| {
                    this.toggle_inspector(cx);
                }))
            })
            // Edit mode: no handler means a disabled menu item. These open
            // overlays that take the keys, change media, or leave the photo
            // under the draft. Rotate and Revert go to the draft; Quit and
            // Open Library stay (they discard the draft and say so).
            .when(!editing && !presenting, |root| {
                root.on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                    this.open_preferences(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                    this.open_about(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowCommandPalette, _w, cx| {
                    this.open_command_palette(cx);
                }))
                .on_action(cx.listener(|this, _: &RescanLibrary, _w, cx| {
                    this.rescan_library(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportMedia, _w, cx| {
                    this.pick_import_folders(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportNetworkVideo, _w, cx| {
                    this.open_network_import_dialog(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleDownloads, _w, cx| {
                    this.toggle_downloads_panel(cx);
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
                .on_action(cx.listener(|this, _: &DeleteRejected, _w, cx| {
                    this.delete_rejected(cx);
                }))
                .on_action(cx.listener(|this, _: &Compare, _w, cx| {
                    this.open_compare(cx);
                }))
                .on_action(cx.listener(|this, _: &Slideshow, window, cx| {
                    this.start_slideshow(window, cx);
                }))
                .on_action(cx.listener(|this, _: &ImportSmartAlbumRules, _w, cx| {
                    this.import_smart_album_rules(cx);
                }))
                .on_action(cx.listener(|this, _: &GoBack, _w, cx| {
                    this.navigate_back(cx);
                }))
                .on_action(cx.listener(|this, _: &GoForward, _w, cx| {
                    this.navigate_forward(cx);
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
            .when(!presenting, |root| {
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
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    // Any click outside the search field should return keyboard
                    // shortcuts to the app. The field's own click re-focuses it.
                    this.blur_search(cx);
                }),
            )
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
            .bg(theme::bg_viewer())
            .flex()
            .flex_col()
            .relative()
            // 0. Recoverable error banner (if any)
            .child(self.render_error_banner(cx))
            // 1. Top 52px Toolbar
            .child(self.render_toolbar(&view_label, window, cx))
            // 2. Workspace Horizontal Split: Sidebar + Resizer + Canvas + Year Rail + Resizer + Inspector
            .child(
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
                            .hover(|s| s.bg(theme::border_strong()))
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
                    // Center Canvas: the browser grid is the only non-modal
                    // content. The media viewer is painted as a fullscreen
                    // overlay above it (see render_viewer_modal).
                    // The filter and status bars belong to this column, so
                    // the sidebar and inspector run full height.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .children(self.render_trash_note())
                            .child(
                                div()
                                    .relative()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .flex_row()
                                    .child(if self.selection == AlbumSelection::Duplicates {
                                        crate::views::stacks::render_duplicates(
                                            self,
                                            total_count,
                                            cx,
                                        )
                                    } else {
                                        self.render_browser_grid(total_count, cx).into_any_element()
                                    })
                                    // Pinned Vertical Year Rail
                                    .child(self.render_year_rail(cx))
                                    // Selection dock and toast, over the grid column only
                                    .child(self.render_action_dock(is_fullscreen_viewer, cx))
                                    .child(self.render_toast_overlay(cx)),
                            )
                            .child(self.render_status_bar(total_count, cx)),
                    )
                    // Inspector Resizer Drag Handle (260–380px)
                    .child(if inspector_open && !is_fullscreen_viewer {
                        div()
                            .id("inspector-resizer")
                            .w(px(4.0))
                            .h_full()
                            .cursor_col_resize()
                            .hover(|s| s.bg(theme::border_strong()))
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
                    .child(if inspector_open && !is_fullscreen_viewer {
                        div()
                            .child(InspectorView::render(self, cx))
                            .into_any_element()
                    } else {
                        Empty.into_any_element()
                    }),
            )
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
            // 10. Tag Manager Modal Overlay
            .child(motion::overlay_in(
                self.tag_manager.open,
                self.render_tag_manager_modal(cx),
                "tag-manager-in",
            ))
            // 11. Keyboard Shortcuts Cheat Sheet
            .child(motion::overlay_in(
                self.shortcuts_overlay_open,
                self.render_shortcuts_overlay(cx),
                "shortcuts-in",
            ))
            // 12. Preferences / About
            .child(motion::overlay_in(
                self.preferences_open,
                crate::views::preferences::render_preferences_modal(self, cx),
                "preferences-in",
            ))
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
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::SHORTCUT_SECTIONS;

    fn says(keys: &str) -> Option<&'static str> {
        SHORTCUT_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .find(|(k, _)| *k == keys)
            .map(|r| r.1)
    }

    /// Regression: the sheet listed "Z / O" for 1:1 zoom, but O opens the
    /// file in its default app (`app/keymap/viewer.rs`).
    #[core::prelude::v1::test]
    fn the_sheet_says_what_z_and_o_do() {
        assert_eq!(says("Z"), Some("Toggle 1:1 zoom"), "Z toggles 1:1 zoom");
        assert_eq!(
            says("O"),
            Some("Open in default app"),
            "O opens the default app, it does not zoom"
        );
    }

    /// Zoom presets moved to ⌘digits; plain digits are reserved for ratings.
    #[core::prelude::v1::test]
    fn the_sheet_lists_zoom_presets_on_command_digits() {
        assert_eq!(says("⌘3"), Some("Fill"), "⌘3 is Fill");
        assert_eq!(
            says("⌘0 / ⌘1 / ⌘2 / ⌘4"),
            Some("Fit / 100% / 200% / 400%"),
            "the presets are on ⌘digits"
        );
        assert_eq!(says("0 / 1 / 2 / 4"), None, "plain digits no longer zoom");
    }

    /// The sheet described Quick Look (there is none), and ← / → as
    /// previous/next even on videos, where they seek.
    #[core::prelude::v1::test]
    fn the_sheet_describes_space_and_arrows_as_they_work() {
        assert_eq!(says("Space / Enter"), Some("Open in viewer"));
        assert_eq!(
            says("← / →"),
            Some("Previous / next photo"),
            "on a video ← / → seek instead"
        );
        for (_, rows) in SHORTCUT_SECTIONS {
            for (keys, what) in rows {
                assert!(
                    !what.contains("Quick look"),
                    "{keys}: there is no Quick Look"
                );
            }
        }
    }
}
