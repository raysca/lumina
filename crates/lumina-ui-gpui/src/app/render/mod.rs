use gpui::*;

use crate::actions::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::InspectorView;
use crate::views::onboarding::OnboardingView;
use crate::views::search_bar::SearchBarView;
use crate::views::sidebar::{AlbumSelection, SidebarView};

mod grid;
mod overlays;
mod rail;
mod status_bar;
mod toolbar;

impl LuminaApp {
    /// Dismissible banner for recoverable, user-facing failures.
    fn render_error_banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.error_message.clone() else {
            return Empty.into_any_element();
        };
        div()
            .id("app-error-banner")
            .w_full()
            .px_3()
            .py_1p5()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(theme::danger_bg())
            .text_xs()
            .text_color(theme::text())
            .child(message)
            .child(
                div()
                    .id("app-error-banner-dismiss")
                    .debug_selector(|| "error-banner-dismiss".to_string())
                    .cursor_pointer()
                    .child(render_icon(Icon::X, px(12.0), theme::text()))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.dismiss_error(cx);
                    })),
            )
            .into_any_element()
    }
}

impl LuminaApp {
    /// Keyboard shortcut cheat sheet (Cmd+/), also serving as keyboard discoverability.
    fn render_shortcuts_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.shortcuts_overlay_open {
            return Empty.into_any_element();
        }
        let rows: [(&str, &str); 26] = [
            ("⌘K", "Command palette"),
            ("⌘F", "Focus search"),
            ("⌘,", "Preferences"),
            ("⌘Z / ⇧⌘Z", "Undo / redo"),
            ("⌘R", "Rescan library"),
            ("⇧⌘R", "Show in Finder"),
            ("⌥⌘C", "Copy file path"),
            ("⌘+ / ⌘-", "Grid density"),
            ("I", "Toggle inspector"),
            ("Space", "Quick look / open viewer"),
            ("Enter", "Open selected item"),
            ("← ↑ → ↓", "Navigate items"),
            ("Home / End", "First / last item"),
            ("PageUp / PageDown", "Jump four rows"),
            ("F", "Favorite"),
            ("T", "Tag selection"),
            ("⌃⌘F", "Full screen viewer"),
            ("⌫", "Delete…"),
            ("Z / O", "Toggle 1:1 zoom"),
            ("J / K / ←→", "Video back 5s / play / ±5s"),
            ("L", "Loop video"),
            ("M", "Mute audio"),
            ("⌥P", "Picture-in-Picture on / off"),
            ("⌥⌘N", "New smart album"),
            ("Esc", "Close / go back"),
            ("⌘/", "Toggle this help"),
        ];
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
                    .bg(theme::bg_pane())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_xl()
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
                    .children(rows.into_iter().map(|(keys, description)| {
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .py_1()
                            .child(div().text_xs().text_color(theme::text()).child(description))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family(theme::FONT_MONO)
                                    .text_color(theme::accent())
                                    .child(keys),
                            )
                    })),
            )
            .into_any_element()
    }
}

impl Render for LuminaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            return div()
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
                .child(OnboardingView::render(
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.pick_folder_dialog(cx);
                    }),
                    cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.open_pictures_library(cx);
                    }),
                    self.has_desktop_pics,
                ))
                .child(self.render_toast_overlay(cx))
                .into_any_element();
        }

        if let Some(scan) = self.first_scan.clone() {
            return div()
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

        let total_count = self.get_active_count();

        let view_label: String = match &self.selection {
            AlbumSelection::AllLibrary => "All Photos & Videos".into(),
            AlbumSelection::System(album) => album.title().into(),
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|a| a.id == *id)
                .map(|a| a.title.clone())
                .unwrap_or_else(|| "Album".into()),
            AlbumSelection::Tag(tag) => format!("#{tag}"),
            AlbumSelection::Untagged => "Untagged".into(),
            AlbumSelection::Trash => "Recently Deleted".into(),
        };

        // The inspector stays while Info is on; with no selection it shows an
        // empty state rather than vanishing under a lit toggle.
        let inspector_open = self.inspector_open;
        let is_fullscreen_viewer = self.is_fullscreen_viewer;

        div()
            .id("lumina-app-root")
            .debug_selector(|| "app-root".to_string())
            .font_family(theme::FONT_SANS)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                this.open_preferences(cx);
            }))
            .on_action(cx.listener(|this, _: &ShowCommandPalette, _w, cx| {
                this.open_command_palette(cx);
            }))
            .on_action(cx.listener(|this, _: &OpenLibrary, _w, cx| {
                this.pick_folder_dialog(cx);
            }))
            .on_action(cx.listener(|this, _: &RescanLibrary, _w, cx| {
                this.rescan_library(cx);
            }))
            .on_action(cx.listener(|this, _: &ShowShortcuts, _w, cx| {
                this.toggle_shortcuts_overlay(cx);
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                this.open_about(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _w, cx| {
                this.toggle_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleInspector, _w, cx| {
                this.toggle_inspector(cx);
            }))
            .on_action(cx.listener(|this, _: &ImportXmp, _w, cx| {
                this.import_sidecars_selected(cx);
            }))
            .on_action(cx.listener(|this, _: &NewSmartAlbum, _w, cx| {
                this.open_album_builder(cx);
            }))
            .on_action(cx.listener(|_this, _: &QuitApp, _w, cx| {
                cx.quit();
            }))
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
            .child(self.render_toolbar(&view_label, cx))
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
                            .hover(|s| s.bg(theme::accent()))
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
                            .child(SearchBarView::render(self, cx))
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .flex()
                                    .flex_row()
                                    .child(self.render_browser_grid(total_count, cx))
                                    // Pinned Vertical Year Rail
                                    .child(self.render_year_rail(cx)),
                            )
                            .child(self.render_status_bar(total_count)),
                    )
                    // Inspector Resizer Drag Handle (260–380px)
                    .child(if inspector_open && !is_fullscreen_viewer {
                        div()
                            .id("inspector-resizer")
                            .w(px(4.0))
                            .h_full()
                            .cursor_col_resize()
                            .hover(|s| s.bg(theme::accent()))
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
            // 5. Floating Multi-Select Action Dock (Bottom Center)
            .child(self.render_action_dock(is_fullscreen_viewer, cx))
            // 5b. Transient success toast (Bottom Right), below modals
            .child(self.render_toast_overlay(cx))
            // 6. Fullscreen Detail Viewer Modal
            .child(self.render_viewer_modal(cx))
            // 7. Visual Smart Album Builder Modal
            .child(self.render_album_builder_modal(cx))
            // 8. Delete / Empty Trash Confirmation Modal
            .child(self.render_delete_confirm_modal(cx))
            // 9. Media Card Context Menu Overlay
            .child(self.render_context_menu_overlay(window, cx))
            // 9b. Sort popover
            .child(crate::views::sort_menu::render_sort_menu_overlay(self, cx))
            // 10. Tag Manager Modal Overlay
            .child(self.render_tag_manager_modal(cx))
            // 11. Keyboard Shortcuts Cheat Sheet
            .child(self.render_shortcuts_overlay(cx))
            // 12. Preferences / About
            .child(crate::views::preferences::render_preferences_modal(
                self, cx,
            ))
            // 13. Command palette (topmost)
            .child(crate::views::command_palette::render_command_palette(
                self, cx,
            ))
            .into_any_element()
    }
}
