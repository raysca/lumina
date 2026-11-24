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
                .text_size(theme::TEXT_LARGE)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(title),
        )
        .children(count.map(|label| {
            div()
                .font_family(theme::FONT_SANS)
                .font_features(theme::tabular_figures())
                .text_size(theme::TEXT_CAPTION)
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
        // Tiles mark themselves again only where a grid paints this frame;
        // a page without one (Albums, Tags, Downloads) must not leave the
        // viewer zooming from where a grid tile used to be.
        crate::motion::forget_tiles();
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

        // Settings is an opaque full-window page, and so is the viewer once
        // its fade-in is over: the library under them is not laid out,
        // refreshed or asked for thumbnails until they close. Video playback
        // repaints the root every frame, so this keeps the library out of it.
        let library_shown = !self.preferences_open && !self.viewer_covers_library(cx);
        // The viewer steps through the active view's items and follows
        // writes made in it (trash, rotate), so that view stays fresh.
        if !self.preferences_open {
            self.refresh_active_view(cx);
        }
        if library_shown {
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
            let inspector_shown = self.docked_inspector_shown();
            #[cfg(test)]
            LIBRARY_BUILDS.with(|n| n.set(n.get() + 1));
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

#[cfg(test)]
thread_local! {
    /// Root renders on this test thread that built the library columns.
    /// (Elements under the viewer report no debug bounds either way.)
    pub(crate) static LIBRARY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl LuminaApp {
    /// Whether the open viewer hides the whole library. During its fade-in
    /// (`motion::OVERLAY_IN`) the library still shows through, so it is
    /// built until the fade is over.
    fn viewer_covers_library(&mut self, cx: &Context<Self>) -> bool {
        if !self.viewer_shown() {
            self.viewer_shown_since = None;
            return false;
        }
        let now = cx.background_executor().now();
        let since = *self.viewer_shown_since.get_or_insert(now);
        now.saturating_duration_since(since) >= crate::motion::OVERLAY_IN
    }

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

    /// An overlay that fades in on open and out on close. `state` is the
    /// field its render reads; while it is closing, the field's last open
    /// value is put back for this one render, then the current one again.
    pub(crate) fn overlay<T: Clone + 'static>(
        &mut self,
        id: &'static str,
        state: fn(&mut Self) -> &mut T,
        open: fn(&T) -> bool,
        render: impl FnOnce(&mut Self, &mut Context<Self>) -> AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = state(self);
        let is_open = open(current);
        let copy = is_open.then(|| current.clone());
        match self.exits.track(id, copy) {
            None => motion::overlay_in(is_open, render(self, cx), id),
            Some((last, fade, started)) => {
                if started {
                    // One frame after the fade ends drops the faded copy.
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(motion::EXIT + std::time::Duration::from_millis(16))
                            .await;
                        this.update(cx, |_, cx| cx.notify()).ok();
                    })
                    .detach();
                }
                let now = std::mem::replace(state(self), last);
                let el = render(self, cx);
                *state(self) = now;
                motion::overlay_out(el, fade)
            }
        }
    }

    /// Settings, over the library or the welcome screen.
    pub(super) fn preferences_layer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.overlay(
            "preferences-in",
            |a| &mut a.preferences_open,
            |open| *open,
            |a, cx| crate::views::preferences::render_preferences_modal(a, cx).into_any_element(),
            cx,
        )
    }

    /// The remove-library confirmation, over Settings and the library.
    pub(super) fn library_delete_layer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        self.overlay(
            "library-delete-in",
            |a| &mut a.library_delete,
            Option::is_some,
            |a, cx| a.render_library_delete_modal(cx).into_any_element(),
            cx,
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
        let root = root
            // 5b. Compare / survey, under the viewer it can open
            .child(crate::views::compare::render(self, cx))
            // 6. Fullscreen Detail Viewer Modal
            .child(self.render_viewer_modal(cx));
        // Each fades in on open and out on close, bottom to top.
        let layers = [
            // 7. Visual Smart Album Builder Modal
            self.overlay(
                "album-builder-in",
                |a| &mut a.album_draft,
                |d| d.modal_open,
                |a, cx| a.render_album_builder_modal(cx).into_any_element(),
                cx,
            ),
            // 8. Delete / Empty Trash Confirmation Modal
            self.overlay(
                "delete-confirm-in",
                |a| &mut a.delete_confirm_state,
                Option::is_some,
                |a, cx| a.render_delete_confirm_modal(cx).into_any_element(),
                cx,
            ),
            // 8b. Smart album delete confirmation
            self.overlay(
                "smart-album-delete-in",
                |a| &mut a.album_draft.confirm_delete,
                Option::is_some,
                |a, cx| crate::views::smart_album_delete::render(a, cx).into_any_element(),
                cx,
            ),
            // 8b2. Album delete confirmation
            self.overlay(
                "album-delete-in",
                |a| &mut a.albums.confirm_delete,
                Option::is_some,
                |a, cx| crate::views::album_delete::render(a, cx).into_any_element(),
                cx,
            ),
            // 8b3. Add to Album picker
            self.overlay(
                "album-picker-in",
                |a| &mut a.albums.picker,
                Option::is_some,
                |a, cx| crate::views::album_picker::render(a, cx).into_any_element(),
                cx,
            ),
            // 8c. Import dialog
            self.overlay(
                "import-dialog-in",
                |a| &mut a.import.dialog,
                Option::is_some,
                |a, cx| crate::views::import_dialog::render(a, cx).into_any_element(),
                cx,
            ),
            // 8d. Network video import dialog
            self.overlay(
                "network-import-dialog-in",
                |a| &mut a.network_import.stage,
                Option::is_some,
                |a, cx| crate::views::network_import_dialog::render(a, cx).into_any_element(),
                cx,
            ),
            // 8d2. Metadata template editor
            self.overlay(
                "template-editor-in",
                |a| &mut a.templates.editor,
                Option::is_some,
                |a, cx| crate::views::template_editor::render(a, cx).into_any_element(),
                cx,
            ),
            // 8e. Downloads dropdown
            self.overlay(
                "downloads-panel-in",
                |a| &mut a.downloads.open,
                |open| *open,
                |a, cx| crate::views::downloads_panel::render(a, cx).into_any_element(),
                cx,
            ),
            // 8f. "Downloads are still running" before quit / library switch
            self.overlay(
                "downloads-confirm-in",
                |a| &mut a.downloads.confirm,
                Option::is_some,
                |a, cx| crate::views::downloads_confirm::render(a, cx).into_any_element(),
                cx,
            ),
            // 9. Media Card Context Menu Overlay
            self.overlay(
                "context-menu-in",
                |a| &mut a.context_menu_state,
                Option::is_some,
                |a, cx| a.render_context_menu_overlay(window, cx).into_any_element(),
                cx,
            ),
            // 9b. Sort popover
            self.overlay(
                "sort-menu-in",
                |a| &mut a.sort_menu_open,
                |open| *open,
                |a, cx| crate::views::sort_menu::render_sort_menu_overlay(a, cx).into_any_element(),
                cx,
            ),
            // 9b2. Album page More menu backdrop (the card hangs from More)
            self.overlay(
                "album-menu-in",
                |a| &mut a.albums.page.menu,
                Option::is_some,
                |a, cx| crate::views::album_page::render_menu_backdrop(a, cx).into_any_element(),
                cx,
            ),
            // 9c. Add filter menu backdrop (the card hangs from its opener)
            self.overlay(
                "filter-menu-in",
                |a| &mut a.filter_menu,
                Option::is_some,
                |a, cx| {
                    crate::views::filter_bar::render_filter_menu_overlay(a, cx).into_any_element()
                },
                cx,
            ),
            // 11. Keyboard Shortcuts Cheat Sheet
            self.overlay(
                "shortcuts-in",
                |a| &mut a.shortcuts_overlay_open,
                |open| *open,
                |a, cx| a.render_shortcuts_overlay(cx).into_any_element(),
                cx,
            ),
            // 12. Settings (About is its last category): a full-window page
            self.preferences_layer(cx),
            // 12a. Remove-library confirmation over Settings (and the library)
            self.library_delete_layer(cx),
        ];
        let root = root
            .children(layers)
            // 12b. The toast over settings (opaque); the columns draw it
            // otherwise.
            .when(self.preferences_open, |root| {
                root.child(self.render_toast_overlay(cx))
            });
        // 13. Command palette (topmost)
        let palette = self.overlay(
            "command-palette-in",
            |a| &mut a.command_palette_open,
            |open| *open,
            |a, cx| crate::views::command_palette::render_command_palette(a, cx).into_any_element(),
            cx,
        );
        root.child(palette)
            // 14. Files dragged in from Finder (shown only during a drag)
            .children(
                (self.grid_state.is_some() && !self.import_blocked_by_modal())
                    .then(|| crate::views::import_dialog::render_drop_target(cx)),
            )
    }
}
