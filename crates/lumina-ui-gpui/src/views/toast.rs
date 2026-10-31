//! Compact toast for transient success feedback, bottom centre over the grid.
//!
//! Toasts are deliberately separate from the top error banner: recoverable
//! failures keep using [`LuminaApp::set_error`], while successful actions get a
//! short confirmation that can be dismissed or undone. Only one toast is shown
//! at a time; a newer toast replaces the previous one and invalidates the older
//! auto-dismiss timer through its generation id.

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// How long a toast stays on screen before dismissing itself.
/// Hovering holds it open; it closes on the first tick after the pointer leaves.
pub const TOAST_DURATION: Duration = Duration::from_secs(8);

/// The single visible toast.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Short user-facing confirmation.
    pub message: String,
    /// True when pressing Undo should run [`LuminaApp::undo_last`].
    pub undo_available: bool,
    /// The pointer is over the toast, so its timer waits.
    pub hovered: bool,
    /// Generation id that pairs this toast with its auto-dismiss timer.
    id: u64,
}

/// Toast state owned by [`LuminaApp`].
#[derive(Debug, Default)]
pub struct ToastState {
    current: Option<Toast>,
    next_id: u64,
}

impl ToastState {
    /// The toast currently on screen, if any.
    pub fn current(&self) -> Option<&Toast> {
        self.current.as_ref()
    }

    /// True when a toast is currently visible.
    pub fn is_visible(&self) -> bool {
        self.current.is_some()
    }

    /// Replaces any visible toast and returns the new generation id.
    fn show(&mut self, message: String, undo_available: bool) -> u64 {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        // A toast replacing one under a resting pointer gets no new hover
        // event (same element id), so it inherits the hover state.
        let hovered = self.current.as_ref().is_some_and(|toast| toast.hovered);
        self.current = Some(Toast {
            message,
            undo_available,
            hovered,
            id,
        });
        id
    }

    /// Clears the visible toast. Returns true when one was showing.
    pub fn dismiss(&mut self) -> bool {
        self.current.take().is_some()
    }

    /// Marks the visible toast hovered or not.
    pub fn set_hovered(&mut self, hovered: bool) {
        if let Some(toast) = self.current.as_mut() {
            toast.hovered = hovered;
        }
    }

    /// Timer tick for toast `id`: clears it unless it was replaced (Done) or
    /// is hovered (Wait, so the timer runs another round).
    fn tick(&mut self, id: u64) -> Tick {
        match self.current.as_ref() {
            Some(toast) if toast.id == id && toast.hovered => Tick::Wait,
            Some(toast) if toast.id == id => {
                self.current = None;
                Tick::Dismissed
            }
            _ => Tick::Done,
        }
    }
}

enum Tick {
    Wait,
    Dismissed,
    Done,
}

impl LuminaApp {
    /// Shows a success toast and replaces any visible toast.
    pub fn show_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.show_toast_inner(message.into(), false, cx);
    }

    /// Shows a success toast whose Undo button runs [`LuminaApp::undo_last`].
    ///
    /// Call this from reversible actions after their undo entry was pushed, so
    /// Undo restores the state the message describes.
    pub fn show_undo_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.show_toast_inner(message.into(), true, cx);
    }

    /// Dismisses the visible toast, if any.
    pub fn dismiss_toast(&mut self, cx: &mut Context<Self>) {
        if self.toast.dismiss() {
            cx.notify();
        }
    }

    /// Applies the toast's Undo and closes it.
    pub fn undo_from_toast(&mut self, cx: &mut Context<Self>) {
        self.dismiss_toast(cx);
        self.undo_last(cx);
    }

    fn show_toast_inner(&mut self, message: String, undo_available: bool, cx: &mut Context<Self>) {
        let id = self.toast.show(message, undo_available);
        // Auto-dismiss on the foreground task so no work blocks the UI thread.
        // The timer only clears the toast it was armed for: replacing a toast
        // leaves a stale task that becomes a no-op when it fires.
        cx.spawn(async move |this, async_cx| loop {
            async_cx.background_executor().timer(TOAST_DURATION).await;
            let tick = this.update(async_cx, |this, cx| {
                let tick = this.toast.tick(id);
                if matches!(tick, Tick::Dismissed) {
                    cx.notify();
                }
                tick
            });
            if !matches!(tick, Ok(Tick::Wait)) {
                break;
            }
        })
        .detach();
        cx.notify();
    }

    /// Toast centred over the grid column, 20px above the status bar, or
    /// stacked above the selection dock when that is showing.
    pub fn render_toast_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.toast.is_visible() {
            return Empty.into_any_element();
        }
        let Some(toast) = self.toast.current() else {
            return Empty.into_any_element();
        };
        let message = toast.message.clone();
        let undo_button = if toast.undo_available {
            div()
                .id("toast-undo")
                .debug_selector(|| "toast-undo".to_string())
                .h(px(24.0))
                .px(px(8.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .rounded(theme::RADIUS_MD)
                .border_1()
                .border_color(theme::selection_border())
                .bg(theme::selection_bg())
                .hover(|style| style.bg(theme::selection_bg_hover()))
                .text_xs()
                .text_color(theme::selection_fg())
                .cursor_pointer()
                .child("Undo")
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.0))
                        .text_color(theme::text_muted())
                        .child("⌘Z"),
                )
                .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.undo_from_toast(cx);
                }))
                .into_any_element()
        } else {
            Empty.into_any_element()
        };

        let dock_showing = (self.selected_item.is_some() || !self.selected_indices.is_empty())
            && !self.is_fullscreen_viewer;
        let toast = div()
            .id("toast")
            .debug_selector(|| "toast".to_string())
            .w(px(360.0))
            .occlude()
            .on_hover(cx.listener(|this, hovered: &bool, _w, _cx| {
                this.toast.set_hovered(*hovered);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px(px(10.0))
            .py(px(8.0))
            .bg(theme::bg_hud())
            .border_1()
            .border_color(theme::border_strong())
            .rounded(theme::RADIUS_LG)
            .shadow_lg()
            .child(render_icon(Icon::Check, px(14.0), theme::success_text()))
            .child(
                div()
                    .id("toast-message")
                    .debug_selector(|| "toast-message".to_string())
                    .flex_1()
                    .text_xs()
                    .text_color(theme::text())
                    .child(message),
            )
            .child(undo_button)
            .child(
                div()
                    .id("toast-dismiss")
                    .debug_selector(|| "toast-dismiss".to_string())
                    .size(px(26.0))
                    .flex_none()
                    .rounded(theme::RADIUS_MD)
                    .border_1()
                    .border_color(theme::border_strong())
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(theme::tooltip_label("Dismiss"))
                    .child(render_icon(Icon::X, px(12.0), theme::text_muted()))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.dismiss_toast(cx);
                    })),
            );
        // ponytail: 48px = dock height (40) + 8px gap; derive it if the dock grows.
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(if dock_showing { 20.0 + 48.0 } else { 20.0 }))
            .when(!self.timeline_year_buckets.is_empty(), |d| {
                d.pr(px(crate::views::grid::YEAR_RAIL_WIDTH))
            })
            .flex()
            .justify_center()
            .child(toast)
            .into_any_element()
    }
}
