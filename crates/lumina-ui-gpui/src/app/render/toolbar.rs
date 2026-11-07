use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::search_bar::SearchBarView;
use crate::views::sidebar::AlbumSelection;

/// 28×28 borderless icon button; a fill shows on hover, or the selection
/// fill while `on`. The tooltip names the action.
fn icon_button(id: &'static str, on: bool) -> Stateful<Div> {
    if on {
        theme::control_on(id).border_color(rgba(0x00000000))
    } else {
        theme::control_ghost(id)
    }
    .w(px(28.0))
    .flex_shrink_0()
    .px_0()
    .justify_center()
}

/// Toolbar icons: 16px, row text colour, the selection colour while on.
fn toolbar_icon(icon: crate::icons::Icon, on: bool) -> AnyElement {
    let color = if on {
        theme::selection_fg()
    } else {
        theme::text_row()
    };
    crate::icons::render_icon(icon, px(16.0), color).into_any_element()
}

#[cfg(test)]
thread_local! {
    /// The test platform always reports server decorations; tests set this to
    /// render the controls a frameless Linux window needs.
    pub(crate) static FORCE_CLIENT_DECORATIONS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Whether the app must draw its own window controls. Wayland compositors
/// without server-side decorations (GNOME) leave the window frameless; macOS
/// and decorating window managers report `Server`.
fn draws_own_window_controls(window: &Window) -> bool {
    #[cfg(test)]
    if FORCE_CLIENT_DECORATIONS.with(|force| force.get()) {
        return true;
    }
    matches!(window.window_decorations(), Decorations::Client { .. })
}

fn window_button(id: &'static str, icon: crate::icons::Icon, label: &'static str) -> Stateful<Div> {
    icon_button(id, false)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(label))
        .child(toolbar_icon(icon, false))
}

/// Minimize, maximize and close for a client-decorated window.
fn window_controls(window: &Window) -> Div {
    let supported = window.window_controls();
    div()
        .ml_2()
        .flex()
        .flex_row()
        .gap_1()
        .when(supported.minimize, |row| {
            row.child(
                window_button("window-minimize", crate::icons::Icon::Minus, "Minimize")
                    .on_click(|_, window, _| window.minimize_window()),
            )
        })
        .when(supported.maximize, |row| {
            let label = if window.is_maximized() {
                "Restore"
            } else {
                "Maximize"
            };
            row.child(
                window_button("window-maximize", crate::icons::Icon::Maximize, label)
                    .on_click(|_, window, _| window.zoom_window()),
            )
        })
        .child(
            window_button("window-close", crate::icons::Icon::X, "Close")
                .on_click(|_, window, _| window.remove_window()),
        )
}

impl LuminaApp {
    pub(crate) fn render_toolbar(
        &self,
        view_label: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_label = view_label.to_string();
        let own_controls = draws_own_window_controls(window);
        let sidebar_toggle_label = format!(
            "{} ({})",
            crate::app::panels::sidebar_toggle_title(self.sidebar_collapsed),
            crate::app::panels::SIDEBAR_SHORTCUT
        );
        // Only Recently Deleted shows a count and its bulk actions.
        let trash_count = if self.selection == AlbumSelection::Trash {
            self.system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .copied()
                .unwrap_or(0)
        } else {
            0
        };
        div()
            .w_full()
            .h(px(52.0))
            .flex_shrink_0()
            // macOS draws the traffic lights over this inset.
            .pl(px(if cfg!(target_os = "macos") {
                84.0
            } else {
                12.0
            }))
            .pr_3()
            .bg(theme::bg_app())
            .border_b_1()
            .border_color(theme::border_subtle())
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(theme::text())
            // Sidebar toggle button
            .child(
                icon_button("sidebar-toggle-btn", false)
                    .debug_selector(|| "sidebar-toggle".to_string())
                    .role(Role::Button)
                    .aria_label(sidebar_toggle_label.clone())
                    .tooltip(theme::tooltip_label(sidebar_toggle_label))
                    .child(toolbar_icon(
                        crate::app::panels::sidebar_toggle_icon(self.sidebar_collapsed),
                        false,
                    ))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sidebar(cx);
                    })),
            )
            // Back/Forward are ⌘[ / ⌘], the Go menu and ⌘K; the design has no
            // toolbar buttons for them.
            // The page title (v3: 15px semibold), then the Trash item count.
            // A long album or tag name truncates instead of pushing the
            // buttons off the bar.
            .child(
                div()
                    .ml_2()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap_1p5()
                    .child(
                        div()
                            .debug_selector(|| "toolbar-title".to_string())
                            .min_w_0()
                            .truncate()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child(if let Some(year) = self.active_year {
                                year.to_string()
                            } else {
                                view_label
                            }),
                    )
                    .when(trash_count > 0, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "trash-item-count".to_string())
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(theme::text_subtle())
                                .child(if trash_count == 1 {
                                    "1 item".to_string()
                                } else {
                                    format!("{trash_count} items")
                                }),
                        )
                    }),
            )
            // Empty toolbar space moves a frameless window; double-click maximizes.
            .child(
                div()
                    .id("toolbar-drag-area")
                    .debug_selector(|| "toolbar-drag-area".to_string())
                    .flex_1()
                    .h_full()
                    .when(own_controls, |area| {
                        area.on_mouse_down(MouseButton::Left, |event, window, _| {
                            if event.click_count >= 2 {
                                window.zoom_window();
                            } else {
                                window.start_window_move();
                            }
                        })
                    }),
            )
            .child(SearchBarView::render_field(self, cx))
            // Trash bulk actions
            .children((trash_count > 0).then(|| {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        theme::control("header-restore-all-btn")
                            .debug_selector(|| "header-restore-all-btn".to_string())
                            .child("Restore All")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.restore_all_trash(cx);
                            })),
                    )
                    .child(
                        theme::control("header-empty-trash-btn")
                            .debug_selector(|| "header-empty-trash-btn".to_string())
                            .bg(theme::danger_bg())
                            .border_color(theme::danger_border())
                            .text_color(theme::danger_text())
                            .child("Empty Trash…")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.prompt_empty_trash(cx);
                            })),
                    )
            }))
            .children(self.render_downloads_button(cx))
            .child(
                icon_button("sort-toolbar-btn", false)
                    .debug_selector(|| "sort-menu-button".to_string())
                    .tooltip(theme::tooltip_label(format!("Sort: {}", self.sort_label())))
                    .child(toolbar_icon(crate::icons::Icon::Sliders, false))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.toggle_sort_menu(cx);
                    })),
            )
            .child(
                icon_button("tags-toolbar-btn", false)
                    .debug_selector(|| "tags-toolbar-btn".to_string())
                    .tooltip(theme::tooltip_label("Tags"))
                    .child(toolbar_icon(crate::icons::Icon::Tag, false))
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_tag_manager(cx);
                    })),
            )
            .child(
                icon_button("inspector-toggle-btn", self.inspector_open)
                    .debug_selector(|| "inspector-toggle".to_string())
                    .tooltip(theme::tooltip_label("Info (I)"))
                    .child(toolbar_icon(crate::icons::Icon::Info, self.inspector_open))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.toggle_inspector(cx);
                    })),
            )
            .when(own_controls, |bar| bar.child(window_controls(window)))
    }
}

/// The toolbar ring: a 2px arc from 12 o'clock, `fraction` of the way round.
fn progress_ring(fraction: f32) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let radius = bounds.size.width.min(bounds.size.height) / 2.0 - px(2.0);
            let center = bounds.center();
            // ponytail: a full circle can't be one arc; stop just short.
            let angle = fraction.clamp(0.0, 0.999) * std::f32::consts::TAU;
            let start = point(center.x, center.y - radius);
            let end = point(
                center.x + radius * angle.sin(),
                center.y - radius * angle.cos(),
            );
            let mut path = PathBuilder::stroke(px(2.0));
            path.move_to(start);
            path.arc_to(
                point(radius, radius),
                px(0.0),
                angle > std::f32::consts::PI,
                true,
                end,
            );
            if let Ok(path) = path.build() {
                window.paint_path(path, theme::accent());
            }
        },
    )
    .absolute()
    .inset_0()
}

impl LuminaApp {
    /// Downloads (⌥⌘L): shown once the list has rows, with the count of
    /// unfinished ones, a red dot for an unseen failure and a progress ring.
    fn render_downloads_button(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if self.downloads.entries.is_empty() {
            return None;
        }
        let open = self.downloads.open;
        let count = self.download_badge_count();
        Some(
            icon_button("downloads-toolbar-btn", open)
                .debug_selector(|| "downloads-toolbar-btn".to_string())
                .relative()
                .tooltip(theme::tooltip_label("Downloads (⌥⌘L)"))
                .child(toolbar_icon(crate::icons::Icon::ArrowDownToLine, open))
                .children(self.download_progress().map(|fraction| {
                    div()
                        .debug_selector(|| "downloads-progress-ring".to_string())
                        .absolute()
                        .inset_0()
                        .child(progress_ring(fraction))
                }))
                .when(count > 0, |el| {
                    el.child(
                        div()
                            .debug_selector(|| "downloads-badge".to_string())
                            .absolute()
                            .top(px(-3.0))
                            .right(px(-3.0))
                            .min_w(px(14.0))
                            .h(px(14.0))
                            .px(px(3.0))
                            .rounded_full()
                            .bg(theme::accent())
                            .text_color(theme::accent_fg())
                            .text_size(px(9.0))
                            .font_weight(FontWeight::BOLD)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(count.to_string()),
                    )
                })
                .when(self.downloads.unseen_failure, |el| {
                    el.child(
                        div()
                            .debug_selector(|| "downloads-failure-dot".to_string())
                            .absolute()
                            .bottom(px(1.0))
                            .right(px(1.0))
                            .size(px(7.0))
                            .rounded_full()
                            .bg(theme::danger()),
                    )
                })
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_downloads_panel(cx);
                })),
        )
    }

    /// "Items stay here until…" above a non-empty Recently Deleted.
    pub(crate) fn render_trash_note(&self) -> Option<Div> {
        let count = self
            .system_album_counts
            .get(&lumina_core::SystemAlbum::RecentlyDeleted)
            .copied()
            .unwrap_or(0);
        (self.selection == AlbumSelection::Trash && count > 0).then(|| {
            div()
                .debug_selector(|| "trash-note".to_string())
                .flex_shrink_0()
                .px_4()
                .py(px(6.0))
                .border_b_1()
                .border_color(theme::border_subtle())
                .text_xs()
                .text_color(theme::text_subtle())
                .child(
                    "Items stay here until you empty the trash. \
                     Restore puts them back in the library.",
                )
        })
    }
}
