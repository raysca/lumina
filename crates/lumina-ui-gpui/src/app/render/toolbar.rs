use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::config::BrowseLayout;
use crate::icons::Icon;
use crate::theme;
use crate::views::page_header::{
    header_icon_button, header_primary_button, page_header, popover_under,
};
use crate::views::sidebar::AlbumSelection;

/// 28×28 borderless icon button; a fill shows on hover, or bg-3 while `on`. The tooltip names the action.
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

/// Toolbar icons: 16px, t2, or t1 while on.
fn toolbar_icon(icon: crate::icons::Icon, on: bool) -> AnyElement {
    let color = if on { theme::t1() } else { theme::t2() };
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
    /// The page header's meta line: the current view's item count and the
    /// months its items span; in Videos "N clips · H h M m · size" instead.
    /// None until a count is known, so a loading view never reads "0 items";
    /// the span (or the Videos totals) joins once its fetch lands.
    pub(crate) fn page_meta(&self) -> Option<String> {
        if self.albums_overview_shown() {
            return Some(self.albums_overview_meta());
        }
        if self.tags_page_shown() {
            return Some(crate::views::tag_manager::meta(self));
        }
        if self.downloads_screen_shown() {
            return self.downloads_meta();
        }
        if self.duplicates_review_shown() {
            return Some(crate::views::duplicates::meta(self));
        }
        if self.view_loading() && self.cached_active_count().is_none() {
            return None;
        }
        let count = self.displayed_count();
        if !self.videos_view() {
            return Some(crate::i18n::items_in_span(count, self.displayed_span()));
        }
        Some(match self.displayed_totals() {
            Some(totals) => crate::i18n::clips_meta(
                count,
                totals.duration_ms,
                &crate::app::types::format_bytes(totals.bytes),
            ),
            None => crate::i18n::clips(count),
        })
    }

    /// The header's Hide/Show Sidebar button; `SIDEBAR_SHORTCUT` is its keyboard path.
    pub(crate) fn render_sidebar_toggle(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let label = format!(
            "{} ({})",
            crate::app::panels::sidebar_toggle_title(self.sidebar_collapsed),
            crate::app::panels::SIDEBAR_SHORTCUT
        );
        icon_button("sidebar-toggle-btn", false)
            .debug_selector(|| "sidebar-toggle".to_string())
            .role(Role::Button)
            .aria_label(label.clone())
            .tooltip(theme::tooltip_label(label))
            .child(toolbar_icon(
                crate::app::panels::sidebar_toggle_icon(self.sidebar_collapsed),
                false,
            ))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.toggle_sidebar(cx);
            }))
    }

    pub(crate) fn render_toolbar(
        &self,
        view_label: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let own_controls = draws_own_window_controls(window);
        // Only Recently Deleted with items shows its bulk actions.
        let trash_has_items = self.selection == AlbumSelection::Trash
            && self
                .system_album_counts
                .get(&lumina_core::SystemAlbum::RecentlyDeleted)
                .is_some_and(|&n| n > 0);
        let title = match self.active_year {
            Some(year) => year.to_string(),
            None => view_label.to_string(),
        };
        let toggle = self.render_sidebar_toggle(cx);
        // Review Duplicates: no grid to sort or filter; one merge action.
        if self.duplicates_review_shown() {
            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .children(self.render_downloads_button(window, cx))
                .when(self.duplicates_to_review(), |bar| {
                    bar.child(crate::views::duplicates::auto_best_button(cx))
                        .child(crate::views::duplicates::skip_all_button(cx))
                        .child(crate::views::duplicates::merge_all_button(self, cx))
                })
                .when(own_controls, |bar| bar.child(window_controls(window)));
            return self.clear_traffic_lights(page_header(
                toggle,
                crate::i18n::t("duplicates.review_title"),
                self.page_meta().map(Into::into),
                actions,
                own_controls,
            ));
        }
        // The Albums overview and the Tags page have their own actions: no
        // grid to sort or filter.
        if self.non_media_page_shown() {
            let actions = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                // Running downloads keep their ring and panel here too.
                .children(self.render_downloads_button(window, cx))
                .when(self.albums_overview_shown(), |bar| {
                    bar.child(crate::views::albums_overview::new_album_button(self, cx))
                })
                .when(self.downloads_screen_shown(), |bar| {
                    bar.child(crate::views::downloads_screen::header_actions(self, cx))
                })
                .when(own_controls, |bar| bar.child(window_controls(window)));
            return self.clear_traffic_lights(page_header(
                toggle,
                title,
                self.page_meta().map(Into::into),
                actions,
                own_controls,
            ));
        }
        // Back/Forward are ⌘[ / ⌘], the Go menu and ⌘K; the design has no
        // header buttons for them.
        let actions = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .children(trash_has_items.then(|| {
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
                            .bg(theme::danger_soft())
                            .border_color(theme::danger_border())
                            .text_color(theme::danger())
                            .child("Empty Trash…")
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.prompt_empty_trash(cx);
                            })),
                    )
            }))
            .children(self.render_view_mode(cx))
            .children(self.render_downloads_button(window, cx))
            .child(
                header_icon_button(
                    "tags-toolbar-btn",
                    Icon::Tag,
                    crate::i18n::t("header.tags"),
                    None,
                    false,
                )
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.open_tag_manager(cx);
                })),
            )
            .child(
                header_icon_button(
                    "inspector-toggle",
                    Icon::Info,
                    crate::i18n::t("header.info"),
                    Some("I"),
                    self.inspector_open,
                )
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_inspector(cx);
                })),
            )
            // Opens the Add filter menu, as the chip row's Add filter does.
            .child(
                header_icon_button(
                    "header-filter",
                    Icon::SlidersHorizontal,
                    crate::i18n::t("header.filter"),
                    None,
                    self.filter_menu_under_header(),
                )
                .relative()
                .when(self.filter_menu_under_header(), |button| {
                    button.child(popover_under(
                        Anchor::TopRight,
                        crate::views::filter_bar::filter_menu_card(self, cx),
                    ))
                })
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_filter_menu(false, cx);
                })),
            )
            .child(
                header_icon_button(
                    "sort-menu-button",
                    Icon::ArrowUpNarrowWide,
                    crate::i18n::t_with("header.sort", &[&self.sort_label()]),
                    None,
                    self.sort_menu_open,
                )
                .relative()
                .when(self.sort_menu_open, |button| {
                    button.child(popover_under(
                        Anchor::TopRight,
                        crate::views::sort_menu::sort_menu_card(self, cx),
                    ))
                })
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.toggle_sort_menu(cx);
                })),
            )
            .child(
                header_primary_button(
                    "header-import",
                    Icon::Import,
                    crate::i18n::t("header.import"),
                    crate::i18n::t("header.import_tooltip"),
                )
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.pick_import_folders(cx);
                })),
            )
            .when(own_controls, |bar| bar.child(window_controls(window)));
        self.clear_traffic_lights(page_header(
            toggle,
            title,
            self.page_meta().map(Into::into),
            actions,
            own_controls,
        ))
    }

    /// The traffic lights sit over the sidebar's brand row; with the
    /// sidebar collapsed to its rail they reach past it into the header.
    fn clear_traffic_lights(&self, header: Div) -> Div {
        header.when(
            cfg!(target_os = "macos") && self.sidebar_collapsed,
            |header| header.pl(px(84.0 - crate::views::grid::SIDEBAR_COLLAPSED_WIDTH)),
        )
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
    /// Mosaic / Grid / Map segments. Mosaic and Grid set
    /// the saved browse layout and leave the map; Map runs Show Map, which
    /// moves to Places, the view the map plots. Not on Recently Deleted or
    /// Duplicates: neither is a view the map could show, and their own
    /// actions fill the header.
    fn render_view_mode(&self, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if matches!(
            self.selection,
            AlbumSelection::Trash | AlbumSelection::Duplicates
        ) {
            return None;
        }
        let (on_mosaic, on_grid, on_map) = self.view_mode_on();
        let segment = |selector: &'static str, icon, label: &str, tooltip: &str, on: bool| {
            theme::segmented_item(selector, on)
                .group(selector)
                .debug_selector(move || selector.to_string())
                .role(Role::Button)
                .aria_label(SharedString::from(label.to_string()))
                .aria_toggled(if on { Toggled::True } else { Toggled::False })
                .tooltip(theme::tooltip_label(tooltip.to_string()))
                .h(px(26.0))
                .px(px(10.0))
                .gap(px(6.0))
                .text_size(px(12.0))
                .font_weight(FontWeight::NORMAL)
                .when(!on, |s| s.text_color(theme::t3()))
                .child(
                    crate::icons::render_icon(
                        icon,
                        px(13.0),
                        if on { theme::t1() } else { theme::t3() },
                    )
                    // The icon lifts with the label on hover.
                    .group_hover(selector, |s| s.text_color(theme::t1())),
                )
                .child(label.to_string())
        };
        Some(
            theme::segmented("view-mode")
                .debug_selector(|| "view-mode".to_string())
                // 32 high like the header buttons: the frame's 3px inset
                // counts its stroke, so 1px border + 2px padding + 26.
                .h(px(32.0))
                .p(px(2.0))
                .rounded(px(9.0))
                // The header fill, not the settings segmented look.
                .bg(theme::bg_2())
                .border_color(theme::line())
                .flex_shrink_0()
                .child(
                    segment(
                        "view-mode-mosaic",
                        Icon::LayoutDashboard,
                        &crate::i18n::t("header.view_mosaic"),
                        &crate::i18n::t("command.show_mosaic"),
                        on_mosaic,
                    )
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.set_browse_layout(BrowseLayout::Mosaic, cx);
                    })),
                )
                .child(
                    segment(
                        "view-mode-grid",
                        Icon::Grid3x3,
                        &crate::i18n::t("header.view_grid"),
                        &crate::i18n::t("command.show_grid"),
                        on_grid,
                    )
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.set_browse_layout(BrowseLayout::Grid, cx);
                    })),
                )
                .child(
                    segment(
                        "view-mode-map",
                        Icon::Map,
                        &crate::i18n::t("header.view_map"),
                        &crate::i18n::t("command.show_map"),
                        on_map,
                    )
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.show_map(cx);
                    })),
                ),
        )
    }

    /// Downloads (⌥⌘L): shown once the list has rows, with the count of
    /// unfinished ones, a red dot for an unseen failure and a progress ring.
    fn render_downloads_button(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let button = header_icon_button(
            "downloads-toolbar-btn",
            Icon::ArrowDownToLine,
            crate::i18n::t("header.downloads"),
            Some("⌥⌘L"),
            self.downloads.open,
        );
        self.downloads_button(button, window, cx)
    }

    /// `button` as the Downloads button: the panel hangs from it, with the
    /// progress ring, the unfinished count and the failure dot. None until
    /// the list has rows. The page header and the album header share it.
    pub(crate) fn downloads_button(
        &self,
        button: Stateful<Div>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        if self.downloads.entries.is_empty() {
            return None;
        }
        let open = self.downloads.open;
        let count = self.download_badge_count();
        Some(
            button
                .relative()
                .when(open, |button| {
                    button.child(popover_under(
                        Anchor::TopRight,
                        crate::views::downloads_panel::panel_card(self, window, cx),
                    ))
                })
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
                            .text_color(theme::on_accent())
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
                            .bottom(px(-2.0))
                            .right(px(-2.0))
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
                .border_color(theme::line())
                .text_xs()
                .text_color(theme::t2())
                .child(
                    "Items stay here until you empty the trash. \
                     Restore puts them back in the library.",
                )
        })
    }
}
