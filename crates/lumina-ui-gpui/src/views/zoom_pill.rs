//! Floating zoom pill over the grid: − and + walk the thumbnail-size ladder
//! (`SIZE_COLUMNS`), as ⌘- / ⌘= do. In Mosaic where the timeline rail is,
//! the Years · Months · Days segments come first (`app/render/bucket_cards.rs`);
//! Moments was dropped (plan 01b, decision 2).
use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::render::overlays::ABOVE_DOCK;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::sidebar::AlbumSelection;

/// Pill height; the toast stacks above it.
pub(crate) const PILL_HEIGHT: f32 = 42.0;
/// Bottom offset above the status bar with no selection dock.
const PILL_BOTTOM: f32 = 24.0;

impl LuminaApp {
    /// The pill's bottom offset in the grid column, or `None` where it is
    /// hidden: the map, Duplicates, an empty or loading view, the viewer.
    pub(crate) fn zoom_pill_bottom(&self) -> Option<f32> {
        let hidden = self.selection == AlbumSelection::Duplicates
            || self.map_shown()
            || self.is_fullscreen_viewer
            || self.displayed_count() == 0;
        (!hidden).then(|| {
            if self.selection_dock_shown() {
                ABOVE_DOCK
            } else {
                PILL_BOTTOM
            }
        })
    }

    pub(crate) fn render_zoom_pill(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(bottom) = self.zoom_pill_bottom() else {
            return Empty.into_any_element();
        };
        let pill = div()
            .id("grid-zoom-pill")
            .debug_selector(|| "grid-zoom-pill".to_string())
            // Clicks on the pill (its padding included) must not reach the
            // grid, which would clear the selection or start a marquee.
            .occlude()
            .h(px(PILL_HEIGHT))
            .px(px(6.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.0))
            .rounded_full()
            .bg(theme::bg_float())
            .border_1()
            .border_color(theme::line_2())
            .shadow_lg()
            .children(self.render_group_segments(cx))
            .child(self.zoom_pill_button("grid-zoom-out", Icon::ZoomOut, -1, cx))
            .child(self.zoom_pill_button("grid-zoom-in", Icon::ZoomIn, 1, cx));
        // The wrapper has no hitbox, so the grid around the pill still gets
        // clicks and marquee drags.
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(bottom))
            // Centre on the grid, not the grid plus the timeline rail.
            .when(self.timeline_rail_shown(), |d| {
                d.pr(px(crate::views::grid::YEAR_RAIL_WIDTH))
            })
            .flex()
            .justify_center()
            .child(pill)
            .into_any_element()
    }

    fn zoom_pill_button(
        &self,
        id: &'static str,
        icon: Icon,
        step: isize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = t(if step > 0 {
            "grid.zoom_in"
        } else {
            "grid.zoom_out"
        });
        let enabled = self.grid_zoom_enabled(step);
        div()
            .id(id)
            .debug_selector(move || id.to_string())
            .role(Role::Button)
            .aria_label(label.clone())
            // GPUI has no aria-disabled; the description says why it does nothing.
            .when(!enabled, |d| d.aria_description(t("grid.zoom_limit")))
            .tooltip(theme::tooltip_label(label))
            .size(px(30.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            // Disabled reads as the dimmer t3 icon with no hover.
            .child(render_icon(
                icon,
                px(15.0),
                if enabled { theme::t2() } else { theme::t3() },
            ))
            .when(enabled, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(theme::bg_3()))
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        if step > 0 {
                            this.zoom_in_grid(cx);
                        } else {
                            this.zoom_out_grid(cx);
                        }
                    }))
            })
    }
}
