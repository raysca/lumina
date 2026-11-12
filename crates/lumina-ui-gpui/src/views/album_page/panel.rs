//! The album settings panel (Design 07 Task 3, frame `w4zpqh`): the right
//! column of an open album while Info is off. It holds KEY PHOTO and
//! Delete Album; the design's SORT ORDER and sharing sections are dropped
//! by decision.

use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::{count, t};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::grid::{DEFAULT_WINDOW_WIDTH, RESIZER_WIDTH};
use crate::views::page_header::text_marker;

/// Width of the panel, its 1px left border included.
pub(crate) const ALBUM_PANEL_WIDTH: f32 = 319.5;
/// Below this window width the panel collapses so the grid keeps room for
/// at least three columns.
const NARROW_BELOW: f32 = 1100.0;

impl LuminaApp {
    /// The panel would show unless the user hid it: an album page with Info
    /// off (the inspector takes the column) in a window wide enough.
    pub(crate) fn album_panel_fits(&self) -> bool {
        let window = self
            .window_bounds
            .map_or(DEFAULT_WINDOW_WIDTH, |(_, _, width, _)| width);
        self.album_page_album().is_some()
            && !self.inspector_open
            && !self.is_fullscreen_viewer
            && window >= NARROW_BELOW
    }

    /// The panel paints: it fits and the user has not hidden it.
    pub(crate) fn album_panel_shown(&self) -> bool {
        self.album_panel_fits() && !self.config.album_panel_hidden
    }

    /// The right column's width as `center_pane_width` takes it: the
    /// inspector, else the panel, else none. That helper adds a resizer to
    /// the column; the panel has none, so it is taken off here.
    pub(crate) fn right_column_width(&self) -> Option<f32> {
        if self.inspector_open && !self.is_fullscreen_viewer {
            Some(self.inspector_width)
        } else if self.album_panel_shown() {
            Some(ALBUM_PANEL_WIDTH - RESIZER_WIDTH)
        } else {
            None
        }
    }

    /// ⌘K Hide / Show Album Settings and View > Toggle Album Settings.
    /// Saved in the config; a failed save puts the panel back and is in the
    /// error banner.
    pub(crate) fn toggle_album_panel(&mut self, cx: &mut Context<Self>) {
        if !self.album_panel_fits() {
            return;
        }
        self.config.album_panel_hidden = !self.config.album_panel_hidden;
        if !self.save_config() {
            self.config.album_panel_hidden = !self.config.album_panel_hidden;
        }
        cx.notify();
    }
}

/// The palette title: what the toggle will do next.
pub(crate) fn toggle_title(hidden: bool) -> String {
    t(if hidden {
        "album_panel.show"
    } else {
        "album_panel.hide"
    })
}

/// The panel, or nothing when it does not show.
pub(crate) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(id) = this.album_page_album().filter(|_| this.album_panel_shown()) else {
        return Empty.into_any_element();
    };
    // The header's count: the stats once read, the grid's until then.
    let n = this
        .album_page_stats(id)
        .map_or(this.displayed_count(), |s| s.photos + s.videos);
    let hint: SharedString = t(if n == 1 {
        "album_panel.hint_one"
    } else {
        "album_panel.hint_other"
    })
    .replace("{n}", &count(n))
    .into();
    let label: SharedString = t("album_panel.delete").into();
    let danger = theme::danger();
    div()
        .debug_selector(|| "album-panel".to_string())
        .flex_none()
        .w(px(ALBUM_PANEL_WIDTH))
        .h_full()
        .bg(theme::bg_1())
        .border_l_1()
        .border_color(theme::line())
        .px(px(18.0))
        .py(px(20.0))
        .flex()
        .flex_col()
        .gap(px(20.0))
        // Sections go above the divider, each with an Inter 9.5/600 t3
        // label; GPUI has no letter-spacing for the design's 1px tracking.
        .child(super::key_photo::section(this, id, cx))
        .child(div().w_full().h(px(1.0)).flex_shrink_0().bg(theme::line()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(
                    div()
                        .id("album-panel-delete")
                        .debug_selector(|| "album-panel-delete".to_string())
                        .role(Role::Button)
                        .aria_label(label.clone())
                        .tooltip(theme::tooltip_label(t("command.delete_album")))
                        .w_full()
                        .h(px(34.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_center()
                        .gap(px(7.0))
                        .rounded(theme::RADIUS_LG)
                        .bg(theme::danger_soft())
                        .hover(|s| s.bg(theme::danger_bg_hover()))
                        .border_1()
                        .border_color(theme::danger_border())
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(danger)
                        .child(render_icon(Icon::Trash, px(13.0), danger))
                        .child(label)
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.prompt_delete_album(id, cx);
                        })),
                )
                .child(
                    div()
                        .text_size(px(10.5))
                        .text_color(theme::t3())
                        .children(text_marker("album-panel-hint", &hint))
                        .child(hint),
                ),
        )
        .into_any_element()
}
