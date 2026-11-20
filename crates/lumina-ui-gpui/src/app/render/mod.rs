use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::*;

use crate::app::LuminaApp;
use crate::motion;
use crate::theme;

pub(crate) mod bucket_cards;
mod card;
mod columns;
pub(crate) mod empty_state;
pub(super) mod error_banner;
mod grid;
mod library_actions;
pub(crate) mod mosaic;
pub(crate) mod overlays;
pub(crate) mod rail;
mod shortcuts;
mod status_bar;
mod toolbar;
mod video_caption;
mod welcome;
#[cfg(test)]
pub(crate) use toolbar::FORCE_CLIENT_DECORATIONS;

/// A section heading with its optional count.
///
/// Month bands and Mosaic bucket cards are the same heading in two contexts;
/// one typography keeps them from drifting on the next design tweak.
pub(crate) fn title_row(title: impl Into<SharedString>, count: Option<SharedString>) -> Div {
    let title: SharedString = title.into();
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(9.0))
        // ponytail: the design's -0.2 tracking is skipped; GPUI has no
        // letter-spacing style.
        .child(
            div()
                .font_family(theme::FONT_DISPLAY)
                .text_size(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(title),
        )
        .children(count.map(|label| {
            div()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.0))
                .text_color(theme::t3())
                .child(label)
        }))
}

/// Menu actions every root handles: the welcome screen, "Building your
/// library" and the library window. Only actions that need no overlay go
/// here; one whose overlay a root does not paint must not be registered there.
fn app_actions(div: Div, cx: &mut Context<LuminaApp>) -> Div {
    div.on_action(cx.listener(|this, _: &QuitApp, _w, cx| {
        this.request_quit(cx);
    }))
    .on_action(cx.listener(|this, _: &CheckForUpdates, _w, cx| {
        this.check_for_updates(true, cx);
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
        // Thumbnails decode for this display's pixels, so the grid never
        // stretches a 1x source over a Retina tile. The texture cache's depth
        // follows the resulting image size to keep its footprint steady; the
        // images it sheds must be retired, not just dropped (the atlas entry
        // outlives the Arc).
        self.window_scale = window.scale_factor();
        let evicted = match self.grid_state.as_ref() {
            Some(state) => state.resize_texture_cache(self.thumbnail_request_size()),
            None => Vec::new(),
        };
        for image in evicted {
            self.retire_image(image);
        }
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
            // Preferences and About paint here: never register an action
            // whose overlay this root does not paint.
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

        // Settings is an opaque full-window page: the library under it is
        // not laid out, refreshed or asked for thumbnails until it closes.
        let library_shown = !self.preferences_open;
        if library_shown {
            self.refresh_active_view(cx);
            self.refresh_top_matches(cx);
            self.refresh_map_points(cx);
            self.prepare_albums_overview(window, cx);
            self.prepare_album_page(cx);
        }
        self.close_header_popovers_under_modals();
        // A mouse-up outside an inactive window never reaches the root.
        if !window.is_window_active() {
            self.query_bar.held_for_click = false;
        }

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
            .relative();
        let root = if library_shown {
            let total_count = self.displayed_count();
            let view_label = self.view_label();
            // The inspector stays while Info is on; with no selection it
            // shows an empty state rather than vanishing under a lit toggle.
            // The Albums overview, the Tags page and the Downloads screen
            // hold no media: the panel would have nothing to show, so it
            // stays hidden there (Info stays on for other views). Review
            // Duplicates hides it too: the compare cards show each copy's
            // facts.
            let inspector_shown = self.inspector_open
                && !self.is_fullscreen_viewer
                && !self.non_media_page_shown()
                && !self.duplicates_review_shown();
            // Sidebar (full height) + resizer + main column (banner,
            // header, canvas, inspector)
            root.child(self.render_library_columns(
                &view_label,
                total_count,
                inspector_shown,
                window,
                cx,
            ))
        } else {
            root
        };
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
            // 12a. Remove-library confirmation over Settings (and the library)
            .child(motion::overlay_in(
                self.library_delete.is_some(),
                self.render_library_delete_modal(cx),
                "library-delete-in",
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
