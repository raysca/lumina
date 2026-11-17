//! Settings › Library's storage card (frame `g52QWL`): what the library
//! takes on disk, as a stacked bar of photos, videos, downloads and the
//! thumbnail cache, a legend, and Clear cache.

use gpui::*;

use super::rows::focus_border;
use crate::app::library_storage::LibraryStorage;
use crate::app::preferences::SettingsButton;
use crate::app::types::format_bytes;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// One part of the bar and the legend.
struct Part {
    name: &'static str,
    label: &'static str,
    color: Rgba,
    bytes: u64,
}

pub(super) fn render_storage_card(
    app: &LuminaApp,
    storage: &LibraryStorage,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let (summary, cache) = (storage.summary, storage.cache.bytes);
    // ponytail: the design names no chart colours; the label blue and
    // purple stand in for its chart-blue and chart-violet.
    let parts = [
        Part {
            name: "photos",
            label: "Photos",
            color: theme::accent(),
            bytes: summary.photo_bytes,
        },
        Part {
            name: "videos",
            label: "Videos",
            color: theme::label_blue(),
            bytes: summary.video_bytes,
        },
        Part {
            name: "downloads",
            label: "Downloads",
            color: theme::label_purple(),
            bytes: summary.download_bytes,
        },
        Part {
            name: "cache",
            label: "Cache",
            color: theme::t3(),
            bytes: cache,
        },
    ];
    // "X of Y used" is the volume; without it, the library's own files.
    let used = match storage.capacity {
        Some((total, available)) => format!(
            "{} of {} used",
            format_bytes(total.saturating_sub(available)),
            format_bytes(total)
        ),
        None => format_bytes(storage.library_bytes()),
    };
    // With the volume known, the segments are the library's share of it,
    // matching "X of Y used", and the rest of the bar is the bg-3 track.
    // Without it the bar is a breakdown of these four parts.
    let sum: u64 = parts.iter().map(|p| p.bytes).sum();
    let whole = match storage.capacity {
        Some((total, _)) => total.max(sum),
        None => sum,
    };
    let bar = div()
        .debug_selector(|| "settings-storage-bar".to_string())
        .w_full()
        .h(px(10.0))
        .rounded_full()
        .overflow_hidden()
        .bg(theme::bg_3())
        .flex()
        .flex_row()
        .children(parts.iter().filter(|p| p.bytes > 0).map(|p| {
            let name = p.name;
            div()
                .debug_selector(move || format!("settings-storage-seg-{name}"))
                .h_full()
                .flex_shrink_0()
                .w(relative(p.bytes as f32 / whole as f32))
                .bg(p.color)
        }));
    let legend = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(16.0))
        .children(parts.iter().map(|p| {
            let marker = format!("{} {}", p.label, format_bytes(p.bytes));
            div()
                .debug_selector(move || format!("settings-storage-legend-text:{marker}"))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(div().size(px(8.0)).rounded_full().bg(p.color))
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(theme::t2())
                        .child(p.label),
                )
                .child(
                    div()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.0))
                        .text_color(theme::t3())
                        .child(format_bytes(p.bytes)),
                )
        }));
    // A second press while clearing does nothing; say why.
    let clear_label = if app.preferences_cache_job.clearing {
        "Clearing…".to_string()
    } else {
        format!("Clear cache · {}", format_bytes(cache))
    };
    let clear_marker = clear_label.clone();
    let button = SettingsButton::StorageCardClear;
    let clear = div()
        .id(button.id())
        .debug_selector(move || button.id().to_string())
        .role(Role::Button)
        .aria_label(SharedString::from(clear_label.clone()))
        .h(px(28.0))
        .px(px(10.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(focus_border(app, button, transparent_black()))
        .bg(theme::bg_2())
        .hover(|s| s.bg(theme::bg_3()))
        .cursor_pointer()
        .whitespace_nowrap()
        .text_size(px(12.0))
        .text_color(theme::t1())
        .child(render_icon(Icon::Eraser, px(12.0), theme::t2()))
        .child(
            div()
                .debug_selector(move || format!("settings-storage-clear-cache-text:{clear_marker}"))
                .child(clear_label),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            button.press(this, cx);
        }));

    div()
        .id("settings-storage-card")
        .debug_selector(|| "settings-storage-card".to_string())
        .flex()
        .flex_col()
        .gap(px(13.0))
        .p(px(16.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(theme::line())
        .bg(theme::bg_1())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(render_icon(Icon::HardDrive, px(15.0), theme::t2()))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(13.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t1())
                        .child("Library storage"),
                )
                .child({
                    let marker = used.clone();
                    div()
                        .debug_selector(move || format!("settings-storage-used-text:{marker}"))
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.5))
                        .text_color(theme::t2())
                        .child(used)
                }),
        )
        .child(bar)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(16.0))
                .child(legend)
                .child(clear),
        )
}
