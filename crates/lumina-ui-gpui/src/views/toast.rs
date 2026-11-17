//! Compact toast for transient feedback, bottom centre over the grid.
//!
//! Toasts are deliberately separate from the top error banner: catalog and
//! config write failures keep using [`LuminaApp::set_error`] (AGENTS.md §2),
//! while successful actions get a short confirmation that can be dismissed or
//! undone. Warning and danger kinds are for transient notices that change no
//! data. Only one toast is shown
//! at a time; a newer toast replaces the previous one and invalidates the older
//! auto-dismiss timer through its generation id.

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

/// How long a toast stays on screen before dismissing itself.
/// Hovering holds it open; it closes on the first tick after the pointer leaves.
pub const TOAST_DURATION: Duration = Duration::from_secs(8);

/// What a toast reports; sets its accent bar, icon and action colour.
// ponytail: no Danger kind; write failures go to the banner (AGENTS.md §2).
// Add it with its first transient, data-free failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    /// A success.
    Positive,
    /// A refusal that changed nothing ("close the open dialog first").
    Warning,
}

impl ToastKind {
    fn colour(self) -> Rgba {
        match self {
            Self::Positive => theme::positive(),
            Self::Warning => theme::warn(),
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Positive => Icon::CircleCheck,
            Self::Warning => Icon::TriangleAlert,
        }
    }

    /// Test marker name: `toast-icon-{name}`.
    fn name(self) -> &'static str {
        match self {
            Self::Positive => "positive",
            Self::Warning => "warning",
        }
    }
}

/// The toast's text action.
// ponytail: only Undo has a caller; View/Retry from the design land with
// their first caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAction {
    /// Runs [`LuminaApp::undo_from_toast`].
    Undo,
}

/// The single visible toast.
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: ToastKind,
    /// Short user-facing title.
    pub message: String,
    /// Optional second line under the title.
    pub subtitle: Option<String>,
    pub action: Option<ToastAction>,
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
    fn show(
        &mut self,
        kind: ToastKind,
        message: String,
        subtitle: Option<String>,
        action: Option<ToastAction>,
    ) -> u64 {
        self.next_id = self.next_id.wrapping_add(1);
        let id = self.next_id;
        // A toast replacing one under a resting pointer gets no new hover
        // event (same element id), so it inherits the hover state.
        let hovered = self.current.as_ref().is_some_and(|toast| toast.hovered);
        self.current = Some(Toast {
            kind,
            message,
            subtitle,
            action,
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
        self.show_toast_kind(ToastKind::Positive, message, None, None, cx);
    }

    /// Shows a success toast whose Undo button runs [`LuminaApp::undo_last`].
    ///
    /// Call this from reversible actions after their undo entry was pushed, so
    /// Undo restores the state the message describes.
    pub fn show_undo_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.show_toast_kind(
            ToastKind::Positive,
            message,
            None,
            Some(ToastAction::Undo),
            cx,
        );
    }

    /// Shows a warning toast: an action was refused and nothing changed.
    pub fn show_warning_toast(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.show_toast_kind(ToastKind::Warning, message, None, None, cx);
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

    /// Shows a toast of `kind` with an optional second line and action,
    /// replacing any visible toast.
    pub fn show_toast_kind(
        &mut self,
        kind: ToastKind,
        message: impl Into<String>,
        subtitle: Option<&str>,
        action: Option<ToastAction>,
        cx: &mut Context<Self>,
    ) {
        let id = self
            .toast
            .show(kind, message.into(), subtitle.map(str::to_owned), action);
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
        let kind = toast.kind;
        let colour = kind.colour();
        let message = toast.message.clone();
        let subtitle = toast.subtitle.clone();
        let action = toast.action.map(|ToastAction::Undo| {
            let label = SharedString::from("Undo");
            div()
                .id("toast-action")
                .debug_selector(|| "toast-action".to_string())
                .flex_none()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(colour)
                .cursor_pointer()
                .hover(|style| style.opacity(0.8))
                .tooltip(theme::tooltip_label("Undo (⌘Z)"))
                .role(Role::Button)
                .aria_label("Undo (⌘Z)")
                .children(text_marker("toast-action", &label))
                .child(label)
                .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                    this.undo_from_toast(cx);
                }))
        });

        let toast = div()
            .id("toast")
            .debug_selector(|| "toast".to_string())
            .w(px(480.0))
            .h(px(62.0))
            .relative()
            .overflow_hidden()
            .occlude()
            .on_hover(cx.listener(|this, hovered: &bool, _w, _cx| {
                this.toast.set_hovered(*hovered);
            }))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .px(px(14.0))
            .bg(theme::bg_float())
            .border_1()
            .border_color(theme::line_2())
            .rounded(px(10.0))
            // media_scrim is exactly black at 65 %, the design's toast shadow.
            .shadow(vec![BoxShadow::new(
                px(0.0),
                px(6.0),
                theme::media_scrim().into(),
            )
            .blur_radius(px(12.0))])
            .child(
                div()
                    .debug_selector(|| "toast-accent".to_string())
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(px(3.0))
                    // Inside the 10 px card corner (1 px border).
                    .rounded_tl(px(9.0))
                    .rounded_bl(px(9.0))
                    .bg(colour),
            )
            .child(
                div()
                    .debug_selector(move || format!("toast-icon-{}", kind.name()))
                    .flex_none()
                    .child(render_icon(kind.icon(), px(16.0), colour)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .id("toast-message")
                            .debug_selector(|| "toast-message".to_string())
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::t1())
                            .truncate()
                            .child(message),
                    )
                    .children(subtitle.map(|subtitle| {
                        div()
                            .debug_selector(|| "toast-subtitle".to_string())
                            .text_size(px(11.0))
                            .text_color(theme::t3())
                            .truncate()
                            .child(subtitle)
                    })),
            )
            .children(action)
            .child(
                div()
                    .id("toast-dismiss")
                    .debug_selector(|| "toast-dismiss".to_string())
                    .size(px(20.0))
                    .flex_none()
                    .rounded(theme::RADIUS_MD)
                    .hover(|s| s.bg(theme::bg_3()))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .tooltip(theme::tooltip_label("Dismiss"))
                    .role(Role::Button)
                    .aria_label("Dismiss")
                    .child(render_icon(Icon::X, px(12.0), theme::t3()))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.dismiss_toast(cx);
                    })),
            );
        // The toast is the top layer: above the zoom pill when it shows,
        // else above the dock, else above the Review Duplicates footer (its
        // height from the last layout: it wraps taller when narrow), else
        // at the dock's own offset.
        use crate::app::render::overlays::{ABOVE_DOCK, DOCK_BOTTOM, STACK_GAP};
        // Over settings the grid's pill, dock and rail are hidden.
        let over_grid = !self.preferences_open;
        let bottom = match self.zoom_pill_bottom().filter(|_| over_grid) {
            Some(pill) => pill + crate::views::zoom_pill::PILL_HEIGHT + STACK_GAP,
            None if over_grid && self.selection_dock_shown() => ABOVE_DOCK,
            None if over_grid && self.duplicates_review_shown() && self.duplicates_to_review() => {
                (self.stacks.review.footer_height.get() + STACK_GAP).max(DOCK_BOTTOM)
            }
            None => DOCK_BOTTOM,
        };
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(bottom))
            .when(over_grid && self.timeline_rail_shown(), |d| {
                d.pr(px(crate::views::grid::YEAR_RAIL_WIDTH))
            })
            .flex()
            .justify_center()
            // A replacing toast keeps this element, so it swaps text in place.
            .child(crate::motion::rise_in(
                toast,
                "toast-in",
                crate::motion::TOAST_IN,
            ))
            .into_any_element()
    }
}
