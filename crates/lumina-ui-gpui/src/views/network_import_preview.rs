//! The Import Video from URL preview: what the link turned out to be,
//! shown before it is queued.

use gpui::*;
use lumina_core::net_import::NetworkMediaInfo;

use crate::app::types::{format_bytes, format_clock};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

/// "384 MB": `format_bytes` without a trailing ".0".
pub(crate) fn size_label(bytes: u64) -> String {
    format_bytes(bytes).replace(".0 ", " ")
}

/// The host part of a URL ("www.youtube.com"), for status text.
pub(crate) fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// A div showing `text`, with its cfg(test) text marker.
fn marked(selector: &'static str, text: String) -> Div {
    let text: SharedString = text.into();
    div()
        .debug_selector(move || selector.to_string())
        .children(text_marker(selector, &text))
        .child(text)
}

fn badge(selector: &'static str, text: String) -> Div {
    marked(selector, text)
        .flex_none()
        .h(px(20.0))
        .px(px(6.0))
        .rounded(px(5.0))
        .bg(theme::bg_3())
        .flex()
        .items_center()
        .font_family(theme::FONT_MONO)
        .text_size(px(10.5))
        .text_color(theme::t2())
}

/// The link as typed, no longer editable: the Input field's look with a
/// quieter outline.
pub(crate) fn read_only_field(url: &str) -> Div {
    div()
        .debug_selector(|| "net-import-url-readonly".to_string())
        .w_full()
        .h(px(46.0))
        .px(px(13.0))
        .bg(theme::bg_0())
        .border_1()
        .border_color(theme::line_2())
        .rounded(px(10.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .overflow_hidden()
        .child(render_icon(Icon::Link2, px(15.0), theme::t3()).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_MONO)
                .text_size(px(12.5))
                .text_color(theme::t2())
                .child(url.to_string()),
        )
}

/// The card: placeholder thumbnail with the length, title, host and
/// uploader, container and size badges, and the ready tick.
pub(crate) fn render(info: &NetworkMediaInfo) -> Stateful<Div> {
    // ponytail: no remote thumbnail; a film placeholder.
    let thumb = div()
        .relative()
        .flex_none()
        .w(px(152.0))
        .h(px(86.0))
        .rounded(px(8.0))
        .bg(theme::bg_3())
        .flex()
        .items_center()
        .justify_center()
        .child(render_icon(Icon::Film, px(22.0), theme::t3()))
        .children(info.duration_secs.map(|secs| {
            marked("net-import-preview-duration", format_clock(secs, true))
                .absolute()
                .right(px(6.0))
                .bottom(px(6.0))
                .h(px(18.0))
                .px(px(5.0))
                .rounded(px(4.0))
                // key_photo_scrim (70 % black) is the closest token to the
                // design's 72 % chip.
                .bg(theme::key_photo_scrim())
                .flex()
                .items_center()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .text_color(theme::on_media())
        }));

    // Host, then the uploader when known: no dangling "·" without one.
    let host = host_of(&info.url).to_string();
    let meta = match info.uploader.as_deref().filter(|u| !u.is_empty()) {
        Some(uploader) => format!("{host} · {uploader}"),
        None => host,
    };

    let badges = div()
        .flex()
        .flex_row()
        .gap(px(6.0))
        .child(badge(
            "net-import-preview-format",
            info.extension.to_uppercase(),
        ))
        .children(
            info.expected_bytes
                .map(|bytes| badge("net-import-preview-size", size_label(bytes))),
        );

    div()
        .id("net-import-preview")
        .debug_selector(|| "net-import-preview".to_string())
        .p(px(12.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.0))
        .bg(theme::bg_2())
        .border_1()
        .border_color(theme::line())
        .rounded(px(12.0))
        .child(thumb)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .child(
                    marked("net-import-preview-title", info.title.clone())
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .line_height(px(19.0))
                        .text_color(theme::t1())
                        .line_clamp(2)
                        .text_ellipsis(),
                )
                .child(
                    marked("net-import-preview-meta", meta)
                        .truncate()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.0))
                        .text_color(theme::t2()),
                )
                .child(badges),
        )
        .child(
            div()
                .flex_none()
                .size(px(24.0))
                .rounded_full()
                .bg(theme::positive_soft())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Check, px(13.0), theme::positive())),
        )
}
