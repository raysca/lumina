//! Playback failed (Design 10): the "Can't play this clip" card over the
//! stage, and the compact message PiP shows instead of an empty window.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::render::empty_state::{empty_body, empty_button, empty_glyph, empty_headline};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::Icon;
use crate::theme;
use crate::views::page_header::text_marker;

/// The danger glyph: `danger_soft` fill, `danger_outline` ring, triangle.
pub(crate) fn danger_glyph(size: Pixels) -> Div {
    empty_glyph(size, Icon::TriangleAlert, theme::danger())
        .bg(theme::danger_soft())
        .border_color(theme::danger_outline())
}

/// The card, when the viewer's video failed to load; otherwise nothing.
pub(super) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
    if !this.playback_failed_shown() {
        return None;
    }
    let focus = this.video.failed_focus;
    let focused = |button: Stateful<Div>, index| {
        button.when(focus == Some(index), |b| {
            b.border_color(theme::focus_ring())
        })
    };
    let retry = focused(
        empty_button(
            "playback-retry",
            true,
            t("playback.retry").into(),
            Some(Icon::RefreshCw),
            px(32.0),
        ),
        0,
    )
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.retry_video_load(cx)));
    let reveal = focused(
        empty_button(
            "playback-reveal",
            false,
            t("playback.reveal").into(),
            Some(Icon::Folder),
            px(32.0),
        ),
        1,
    )
    .tooltip(theme::tooltip_label(format!(
        "{} (⇧⌘R)",
        t("playback.reveal")
    )))
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reveal_failed_video(cx)));
    // A file that vanished says so, in the missing card's words.
    let (title, body) = if this.video.load_file_gone {
        (t("viewer.missing_title"), t("viewer.missing_no_preview"))
    } else {
        (t("playback.failed_title"), t("playback.failed_body"))
    };
    let error = this.video.load_error.clone().map(SharedString::from);
    let card = div()
        .id("viewer-playback-failed")
        .debug_selector(|| "viewer-playback-failed".to_string())
        .role(Role::Alert)
        .aria_label(title.clone())
        // Clicks on the card must not reach the stage.
        .occlude()
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, e: &MouseDownEvent, _, cx| {
                super::viewport::open_viewer_menu(this, e.position, cx)
            }),
        )
        .w(px(460.0))
        .h(px(224.0))
        .max_w_full()
        .px(px(24.0))
        .rounded(theme::RADIUS_LG)
        .bg(theme::bg_float())
        .border_1()
        .border_color(theme::line_2())
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(danger_glyph(px(54.0)))
        .child(
            // Design: Inter Tight 17/600 (the shared headline is 18).
            empty_headline("playback-failed-title", title.into())
                .mt(px(12.0))
                .text_size(px(17.0)),
        )
        .child(
            empty_body("playback-failed-body", body.into())
                .mt(px(6.0))
                .text_size(px(12.0))
                .line_height(px(18.0)),
        )
        .child(
            div()
                .mt(px(14.0))
                .flex()
                .flex_row()
                .gap(px(9.0))
                .child(retry)
                .child(reveal),
        );
    Some(
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(card)
            // ponytail: the error shows directly under the card, no "Show
            // details" toggle (as the unreachable library does); one line.
            .children(error.map(|error| {
                div()
                    .debug_selector(|| "playback-failed-error".to_string())
                    .mt(px(18.0))
                    .w(px(460.0))
                    .max_w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis_middle()
                    .text_center()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(10.0))
                    .text_color(theme::t3())
                    .child(error)
            }))
            .into_any_element(),
    )
}

/// PiP's compact failure: glyph, one line and Try Again in one row, in the
/// gap between the fades. A `narrow` window (portrait) drops the line. Only the row takes the press, and never starts a
/// window drag; the rest of the window still drags and closes.
pub(crate) fn pip_message(app: WeakEntity<LuminaApp>, narrow: bool, gone: bool) -> AnyElement {
    // A file that vanished says so, as the viewer card does.
    let title: SharedString = t(if gone {
        "viewer.missing_title"
    } else {
        "playback.failed_title"
    })
    .into();
    let retry = empty_button(
        "pip-playback-retry",
        true,
        t("playback.retry").into(),
        None,
        px(26.0),
    )
    .on_click(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        if let Some(app) = app.upgrade() {
            app.update(cx, |app, cx| app.retry_video_load(cx));
        }
    });
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("pip-playback-failed")
                .debug_selector(|| "pip-playback-failed".to_string())
                .role(Role::Alert)
                .aria_label(title.clone())
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .child(danger_glyph(px(30.0)))
                .when(!narrow, |row| {
                    row.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(theme::on_media())
                            .whitespace_nowrap()
                            .children(text_marker("pip-playback-failed-title", &title))
                            .child(title),
                    )
                })
                .child(retry),
        )
        .into_any_element()
}
