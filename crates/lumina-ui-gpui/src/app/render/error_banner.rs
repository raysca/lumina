//! The error banner: the app's API for recoverable failures and its render.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

impl LuminaApp {
    /// Shows the error banner for a recoverable failure. "Title: detail"
    /// splits into a title line and a description.
    pub fn set_error(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.error_message = Some(message.into());
        self.error_retry = None;
        cx.notify();
    }

    /// Like [`Self::set_error`], with a Retry button (and ⌘K "Retry Failed
    /// Action") that runs `retry`.
    pub fn set_error_with_retry(
        &mut self,
        message: impl Into<String>,
        retry: ErrorRetry,
        cx: &mut Context<Self>,
    ) {
        self.set_error(message, cx);
        self.error_retry = Some(retry);
    }

    /// Clears the banner and runs its retry, if it has one.
    pub fn retry_error(&mut self, cx: &mut Context<Self>) {
        if let Some(retry) = self.error_retry.take() {
            self.dismiss_error(cx);
            retry(self, cx);
        }
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &self.grid_state {
            state.engine.rearm_read_errors();
        }
        self.error_message = None;
        self.error_retry = None;
        self.import.failure = None;
        cx.notify();
    }
}

/// The action an error banner's Retry re-runs.
pub type ErrorRetry = fn(&mut LuminaApp, &mut Context<LuminaApp>);

impl LuminaApp {
    /// Banner for recoverable failures: alert-circle, title and
    /// optional detail in `on_danger`, optional Retry, and a text Dismiss.
    pub(crate) fn render_error_banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(message) = self.error_message.clone() else {
            return Empty.into_any_element();
        };
        let (title, detail) = match message.split_once(": ") {
            Some((title, detail)) => (title.to_string(), Some(detail.to_string())),
            None => (message, None),
        };
        // A later error took the banner: the import's failure line stays
        // under it, so which files were not copied is not lost.
        let import_failure = self
            .import
            .failure
            .clone()
            .filter(|failure| self.error_message.as_ref() != Some(failure));
        let dismiss: SharedString = t("error_banner.dismiss").into();
        div()
            .id("app-error-banner")
            .debug_selector(|| "error-banner".to_string())
            .w_full()
            .min_h(px(43.5))
            .px_4()
            .when(detail.is_some() || import_failure.is_some(), |d| d.py_2())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(9.0))
            .bg(theme::danger_surface())
            .border_b_1()
            .border_color(theme::danger_border())
            .child(render_icon(Icon::CircleAlert, px(16.0), theme::danger()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .text_color(theme::on_danger())
                    .child(
                        div()
                            .debug_selector(|| "error-banner-title".to_string())
                            .text_size(px(12.5))
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .children(detail.map(|detail| {
                        div()
                            .debug_selector(|| "error-banner-detail".to_string())
                            .text_xs()
                            .child(detail)
                    }))
                    .children(import_failure.map(|failure| {
                        div()
                            .debug_selector(|| "error-banner-import".to_string())
                            .text_xs()
                            .child(failure)
                    })),
            )
            .children(self.error_retry.is_some().then(|| {
                theme::control("app-error-banner-retry")
                    .debug_selector(|| "error-banner-retry".to_string())
                    .tooltip(theme::tooltip_label("Retry (⌘K Retry Failed Action)"))
                    .child("Retry")
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.retry_error(cx);
                    }))
            }))
            .child(
                div()
                    .id("app-error-banner-dismiss")
                    .debug_selector(|| "error-banner-dismiss".to_string())
                    .role(Role::Button)
                    .aria_label(dismiss.clone())
                    .flex_none()
                    .cursor_pointer()
                    .px(px(6.0))
                    .py(px(4.0))
                    .rounded(theme::RADIUS_SM)
                    .text_size(px(11.5))
                    .line_height(px(14.0))
                    .text_color(theme::t2())
                    .hover(|s| s.text_color(theme::t1()))
                    .tooltip(theme::tooltip_label(t("error_banner.dismiss_tooltip")))
                    .children(text_marker("error-banner-dismiss", &dismiss))
                    .child(dismiss)
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.dismiss_error(cx);
                    })),
            )
            .into_any_element()
    }
}
