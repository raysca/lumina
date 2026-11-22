//! The unreachable-library state shown on the welcome screen
//! when the startup or chosen library is not where it was.

use crate::app::render::empty_state::{empty_body, empty_button, empty_glyph, empty_headline};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::onboarding::WelcomeHandler;
use crate::views::page_header::text_marker;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// The unreachable-library state: Retry or choose another.
pub struct MissingLibraryProps {
    pub path: String,
    /// Says why, in words that stay true for this path.
    pub body: String,
    /// Retry is checking in the background: "Checking…", not clickable.
    pub checking: bool,
    /// Why the last Retry could not open it, when there is a real error.
    pub error: Option<String>,
    /// Tab focus: 0 Retry, 1 Choose.
    pub focus: Option<usize>,
    pub on_retry: WelcomeHandler,
    pub on_choose: WelcomeHandler,
    /// Back to the welcome screen (Esc).
    pub on_back: WelcomeHandler,
}

/// The last or chosen library is not where it was: the centred state in
/// place of the welcome content. ↵ or Retry checks again off the UI thread;
/// ⌘O or Choose picks another folder; Tab moves between the two. Esc or
/// Back returns to the welcome screen and its recent libraries.
pub(crate) fn missing_library_state(props: MissingLibraryProps) -> AnyElement {
    let MissingLibraryProps {
        path,
        body,
        checking,
        error,
        focus,
        on_retry,
        on_choose,
        on_back,
    } = props;
    let title = t("welcome.missing_title");
    let alert = format!("{title}. {path}. {body}");
    let path = SharedString::from(path);
    let focused = |button: Stateful<Div>, index| {
        button.when(focus == Some(index), |b| {
            b.border_color(theme::focus_ring())
        })
    };
    let retry_label = if checking {
        t("welcome.missing_checking")
    } else {
        t("welcome.missing_retry")
    };
    let retry = focused(
        empty_button(
            "missing-library-retry",
            true,
            retry_label.into(),
            Some(Icon::RefreshCw),
            px(34.0),
        ),
        0,
    )
    .tooltip(theme::tooltip_label(format!(
        "{} (↵)",
        t("welcome.missing_retry")
    )));
    // While checking it says so and ignores clicks; a second open would race.
    let retry = if checking {
        retry.opacity(0.6).cursor_default()
    } else {
        retry.on_click(on_retry)
    };
    let choose = focused(
        empty_button(
            "missing-library-choose",
            false,
            t("welcome.missing_choose").into(),
            Some(Icon::Folder),
            px(34.0),
        ),
        1,
    )
    .tooltip(theme::tooltip_label(format!(
        "{} (⌘O)",
        t("welcome.missing_choose")
    )))
    .on_click(on_choose);

    div()
        .size_full()
        .bg(theme::bg_0())
        .text_color(theme::t1())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px(px(28.0))
        .py(px(20.0))
        .child(
            div()
                .id("welcome-missing-library")
                .debug_selector(|| "welcome-missing-library".to_string())
                .role(Role::Alert)
                .aria_label(alert)
                .children(cfg!(test).then(|| {
                    div()
                        .absolute()
                        .size_0()
                        .debug_selector(|| "welcome-missing-library-role:alert".to_string())
                }))
                .w_full()
                .max_w(px(450.0))
                .flex()
                .flex_col()
                .items_center()
                .child(
                    empty_glyph(px(54.0), Icon::Unplug, theme::danger())
                        .bg(theme::danger_soft())
                        .border_color(theme::danger_outline()),
                )
                .child(empty_headline("missing-library-headline", title.into()))
                .child(
                    div()
                        .debug_selector(|| "missing-library-path".to_string())
                        .mt(px(10.0))
                        .h(px(30.0))
                        .max_w_full()
                        .px(px(11.0))
                        .rounded(px(6.0))
                        .bg(theme::bg_1())
                        .border_1()
                        .border_color(theme::line())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(7.0))
                        .children(text_marker("missing-library-path", &path))
                        .child(render_icon(Icon::HardDrive, px(12.0), theme::t3()).flex_none())
                        // A long path loses its middle, keeping the volume and the folder.
                        .child(
                            div()
                                .debug_selector(|| "missing-library-path-line".to_string())
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis_middle()
                                .font_family(theme::FONT_SANS)
                                .font_features(theme::tabular_figures())
                                .text_size(theme::TEXT_CAPTION)
                                .text_color(theme::t2())
                                .child(path),
                        ),
                )
                .child(empty_body("missing-library-body", body.into()).mt(px(14.0)))
                .child(
                    div()
                        .mt(px(16.0))
                        .flex()
                        .flex_row()
                        .justify_center()
                        .gap(px(9.0))
                        .child(retry)
                        .child(choose),
                )
                .child(
                    div()
                        .id("missing-library-back")
                        .debug_selector(|| "missing-library-back".to_string())
                        .role(Role::Button)
                        .aria_label(t("welcome.missing_back"))
                        .mt(px(14.0))
                        .px(px(6.0))
                        .rounded(theme::RADIUS_MD)
                        .cursor_pointer()
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme::t3())
                        .hover(|s| s.text_color(theme::t1()))
                        .tooltip(theme::tooltip_label(format!(
                            "{} (Esc)",
                            t("welcome.missing_back")
                        )))
                        .on_click(on_back)
                        .child(t("welcome.missing_back")),
                )
                // ponytail: the error shows directly, no "Show details"
                // toggle; it is one line and appears only after a failed open.
                .children(error.map(|error| {
                    div()
                        .debug_selector(|| "missing-library-error".to_string())
                        .mt(px(14.0))
                        .max_w_full()
                        .font_family(theme::FONT_SANS)
                        .font_features(theme::tabular_figures())
                        .text_size(theme::TEXT_CAPTION)
                        .text_color(theme::t3())
                        .text_center()
                        .child(error)
                })),
        )
        .into_any_element()
}
