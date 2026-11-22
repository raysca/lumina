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

/// Splits "Title: detail" into the banner's two lines. The detail is
/// usually an error chain ("creating an album: database is locked");
/// leading links that only restate the title ("Could not create album")
/// are dropped, so the detail says what the title does not. A link that
/// adds anything (a path, another noun) stays.
fn banner_parts(message: &str) -> (String, Option<String>) {
    let Some((title, detail)) = message.split_once(": ") else {
        return (message.to_string(), None);
    };
    let title_lower = title.to_lowercase();
    // "could not create album" -> "creat": matches "creating …".
    let stem = title_lower
        .strip_prefix("could not ")
        .and_then(|rest| rest.split_whitespace().next())
        .map(|verb| verb.strip_suffix('e').unwrap_or(verb));
    let title_words: std::collections::HashSet<&str> = title_lower.split_whitespace().collect();
    // The verb's -ing form, then only articles and the title's own words.
    let restates = |link: &str| {
        let link = link.to_lowercase();
        if link == title_lower {
            return true;
        }
        let mut words = link.split_whitespace();
        let verb = words.next().is_some_and(|word| {
            stem.is_some_and(|stem| word.starts_with(stem) && word.ends_with("ing"))
        });
        verb && words.all(|word| matches!(word, "a" | "an" | "the") || title_words.contains(word))
    };
    let mut rest = detail;
    // Always keep the last link: it is the cause.
    while let Some((link, tail)) = rest.split_once(": ") {
        if !restates(link) {
            break;
        }
        rest = tail;
    }
    (title.to_string(), Some(rest.to_string()))
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
        let (title, detail) = banner_parts(&message);
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
                            .text_size(theme::TEXT_SMALL)
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .children(detail.map(|detail| {
                        div()
                            .debug_selector(|| "error-banner-detail".to_string())
                            .text_size(theme::TEXT_SMALL)
                            .child(detail)
                    }))
                    .children(import_failure.map(|failure| {
                        div()
                            .debug_selector(|| "error-banner-import".to_string())
                            .text_size(theme::TEXT_SMALL)
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
                    .text_size(theme::TEXT_SMALL)
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

#[cfg(test)]
mod tests {
    use super::banner_parts;

    #[core::prelude::v1::test]
    fn banner_detail_drops_links_that_restate_the_title() {
        let parts = banner_parts;
        assert_eq!(
            parts("Could not create album: creating an album: database is locked"),
            (
                "Could not create album".to_string(),
                Some("database is locked".to_string())
            ),
            "a context link that restates the title is not shown under it"
        );
        assert_eq!(
            parts("Could not read edits: could not read edits: no such row").1,
            Some("no such row".to_string()),
            "a link equal to the title is dropped"
        );
        assert_eq!(
            parts("Could not save settings: could not write /tmp/c.json: disk full").1,
            Some("could not write /tmp/c.json: disk full".to_string()),
            "context that adds information (what was written) stays"
        );
        assert_eq!(
            parts("Could not open library: opening /Volumes/X/lib: no such file").1,
            Some("opening /Volumes/X/lib: no such file".to_string()),
            "a link that restates the verb but adds a path stays"
        );
        assert_eq!(
            parts("Could not open library: opening the library: no such file").1,
            Some("no such file".to_string()),
            "a link with only the verb, articles and the title's words is dropped"
        );
        assert_eq!(
            parts("Could not create album: creating album cover: disk full").1,
            Some("creating album cover: disk full".to_string()),
            "a link naming something the title does not stays"
        );
        assert_eq!(
            parts("Could not delete: deleting").1,
            Some("deleting".to_string()),
            "the last link (the cause) always stays"
        );
        assert_eq!(parts("Plain failure"), ("Plain failure".to_string(), None));
    }
}
