//! The library window's sidebar, main column and inspector.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::theme;
use crate::views::inspector::InspectorView;
use crate::views::sidebar::{AlbumSelection, SidebarView};

impl LuminaApp {
    /// The sidebar, its resizer and the main column: banner, header, the
    /// canvas with its bars, and the inspector or album panel.
    pub(super) fn render_library_columns(
        &mut self,
        view_label: &str,
        total_count: usize,
        inspector_shown: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        // The fullscreen viewer covers the header and the canvas: leave their
        // controls out of the tree, as the sidebar does (VoiceOver listed
        // them under the viewer). The viewer reads items without the grid.
        let covered = self.viewer_shown();
        // While the panels slide, each is clipped to its slid width and
        // keeps its content at full width, so nothing reflows mid-way.
        let sliding = self.panel_slide.active();
        if sliding {
            window.request_animation_frame();
        }
        let (sidebar_slack, right_slack) = self.panel_slide.slack();
        let (sidebar_w, right_w) = self.panel_widths();
        let slid = |el: Div, width: f32| {
            if sliding {
                div()
                    .flex_none()
                    .h_full()
                    .w(px(width.max(0.0)))
                    .overflow_hidden()
                    .bg(theme::bg_1())
                    .child(el.flex_none())
            } else {
                el
            }
        };
        // A closing inspector keeps its content while it slides shut.
        let inspector_drawn = inspector_shown || (sliding && right_slack > 0.0);
        let sidebar = div()
            .h_full()
            .flex()
            .flex_row()
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
            });
        div()
            .flex_1()
            .size_full()
            .flex()
            .flex_row()
            .overflow_hidden()
            .child(slid(sidebar, sidebar_w + sidebar_slack))
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
                            .child(self.render_import_banner(cx))
                    })
                    .child(if covered {
                        Empty.into_any_element()
                    } else if self.search_screen_shown() {
                        self.render_search_header(cx).into_any_element()
                    } else if let Some(id) = self.album_page_album() {
                        crate::views::album_page::header::render(self, id, window, cx)
                            .into_any_element()
                    } else {
                        self.render_toolbar(view_label, window, cx)
                            .into_any_element()
                    })
                    .when(!covered, |col| {
                        col.child(crate::views::album_page::selection_bar::render(
                            self, window, cx,
                        ))
                    })
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
                                            .child(if covered {
                                                div().flex_1().into_any_element()
                                            } else if self.selection == AlbumSelection::Duplicates {
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
                                            })
                                            // Pinned Vertical Year Rail
                                            .when(!covered, |d| d.child(self.render_year_rail(cx)))
                                            // Zoom pill, selection dock and toast, over the grid
                                            // column only, stacked bottom-up: dock, pill, toast.
                                            // The viewer covers all but the toast.
                                            .when(!covered, |d| {
                                                d.child(self.render_zoom_pill(cx))
                                                    .child(self.render_action_dock(cx))
                                            })
                                            .when(!self.preferences_open, |d| {
                                                d.child(self.render_toast_overlay(cx))
                                            }),
                                    )
                                    .child(self.render_status_bar(total_count, cx))
                                    .into_any_element()
                            })
                            .child(slid(
                                div()
                                    .h_full()
                                    .flex()
                                    .flex_row()
                                    // Inspector Resizer Drag Handle (260–380px)
                                    .child(if inspector_drawn {
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
                                    .child(if inspector_drawn {
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
                                right_w + right_slack,
                            )),
                    ),
            )
    }
}
