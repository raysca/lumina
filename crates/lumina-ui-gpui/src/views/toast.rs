//! Compact bottom-right toast for transient success feedback.
//!
//! Toasts are deliberately separate from the top error banner: recoverable
//! failures keep using [`LuminaApp::set_error`], while successful actions get a
//! short confirmation that can be dismissed or undone. Only one toast is shown
//! at a time; a newer toast replaces the previous one and invalidates the older
//! auto-dismiss timer through its generation id.

use std::time::Duration;

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// How long a toast stays on screen before dismissing itself.
pub const TOAST_DURATION: Duration = Duration::from_secs(4);

/// The single visible toast.
#[derive(Debug, Clone)]
pub struct Toast {
    /// Short user-facing confirmation.
    pub message: String,
    /// True when pressing Undo should run [`LuminaApp::undo_last`].
    pub undo_available: bool,
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
        self.current = Some(Toast {
            message,
            undo_available,
            id,
        });
        id
    }

    /// Clears the visible toast. Returns true when one was showing.
    pub fn dismiss(&mut self) -> bool {
        self.current.take().is_some()
    }

    /// Clears the toast only when it is still `id`, so a replaced toast's timer
    /// cannot dismiss its successor. Returns true when the toast was cleared.
    fn dismiss_if_current(&mut self, id: u64) -> bool {
        if self.current.as_ref().map(|toast| toast.id) == Some(id) {
            self.current = None;
            true
        } else {
            false
        }
    }
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
        cx.spawn(async move |this, async_cx| {
            async_cx.background_executor().timer(TOAST_DURATION).await;
            let _ = this.update(async_cx, |this, cx| {
                if this.toast.dismiss_if_current(id) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Bottom-right toast overlay, painted above the grid and below modals.
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
                .rounded(px(6.0))
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
                        .text_size(px(10.0))
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

        div()
            .id("toast")
            .debug_selector(|| "toast".to_string())
            .absolute()
            .right(px(16.0))
            .bottom(px(36.0))
            .w(px(320.0))
            .occlude()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px(px(10.0))
            .py(px(8.0))
            .bg(theme::bg_hud())
            .border_1()
            .border_color(theme::border_strong())
            .rounded(px(8.0))
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
                    .size(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(theme::tooltip_label("Dismiss"))
                    .child(render_icon(Icon::X, px(12.0), theme::text_muted()))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.dismiss_toast(cx);
                    })),
            )
            .into_any_element()
    }
}
