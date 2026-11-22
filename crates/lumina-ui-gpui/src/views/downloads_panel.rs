//! The downloads dropdown under the toolbar button: the Active group
//! (downloading, paused, waiting, failed), a divider, then Earlier.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::net_import::NetworkDownloadProgress;
use lumina_core::DownloadState;

use crate::app::downloads::DownloadSurface;
use crate::theme;
use crate::views::{downloads_popover, downloads_popover_rows};
use crate::LuminaApp;

/// "2 min left", from what is left and the current speed.
pub(crate) fn time_left(progress: &NetworkDownloadProgress) -> Option<String> {
    let total = progress.total_bytes?;
    if progress.speed_bytes_per_sec == 0 || total <= progress.bytes_downloaded {
        return None;
    }
    let secs = (total - progress.bytes_downloaded) / progress.speed_bytes_per_sec;
    Some(match secs {
        0..=59 => "under a minute left".to_string(),
        60..=3599 => format!("{} min left", secs / 60),
        _ => format!("{} h {} min left", secs / 3600, (secs % 3600) / 60),
    })
}

/// The dismiss backdrop behind the downloads list. The list itself hangs
/// from the header's Downloads button (`panel_card`), which shows whenever
/// there are rows; the list only opens, and stays open, while there are.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !app.downloads.open {
        return Empty.into_any_element();
    }
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .id("downloads-backdrop")
                .debug_selector(|| "downloads-backdrop".to_string())
                // Covers the toolbar too, so a second click on the icon
                // closes the list rather than reopening it.
                .occlude()
                .absolute()
                .inset_0()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.close_downloads_panel(cx);
                    }),
                ),
        )
        .into_any_element()
}

/// The downloads popover: 420 wide (less on a narrow window), at most 550
/// high. Header and footer stay put; only the rows scroll.
pub fn panel_card(app: &LuminaApp, window: &Window, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let first_waiting = app
        .downloads
        .entries
        .iter()
        .find(|e| e.row.state == DownloadState::Waiting)
        .map(|e| e.row.id);
    // Each Earlier head and row is a direct child: `scroll_to_item` counts
    // them (`move_download_focus`), so nothing else may go in here. The
    // Active group has no head; the divider belongs to the Earlier head.
    let mut body = div()
        .id("downloads-panel-body")
        .debug_selector(|| "downloads-panel-body".to_string())
        .role(Role::List)
        .aria_label(crate::i18n::t("downloads.title"))
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&app.downloads.scroll)
        .p(px(8.0))
        .flex()
        .flex_col()
        .gap(px(4.0));
    let sections = app.download_sections(DownloadSurface::Panel);
    let any_active = sections.iter().any(|(label, _)| *label == "Active");
    for (label, rows) in sections {
        let earlier = label == "Earlier";
        if earlier {
            body = body.child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .when(any_active, |el| {
                        el.child(
                            div()
                                .debug_selector(|| "downloads-divider".to_string())
                                .h(px(1.0))
                                .bg(theme::line()),
                        )
                    })
                    .child(downloads_popover::earlier_head(cx)),
            );
        }
        for entry in rows {
            body = body.child(if earlier {
                downloads_popover_rows::earlier_row(app, entry, cx)
            } else {
                downloads_popover_rows::active_row(
                    app,
                    entry,
                    first_waiting == Some(entry.row.id),
                    cx,
                )
            });
        }
    }

    let viewport = window.viewport_size();
    div()
        .debug_selector(|| "downloads-panel".to_string())
        .id("downloads-panel")
        .role(Role::Dialog)
        .aria_label(crate::i18n::t("downloads.title"))
        .occlude()
        .w(px(420.0).min(viewport.width - px(32.0)))
        // Room for the toolbar above it.
        .max_h(px(550.0).min(viewport.height - px(80.0)))
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line_2())
        .rounded(theme::RADIUS_XL)
        .shadow_2xl()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(downloads_popover::header(app, cx))
        .child(body)
        .child(downloads_popover::footer(cx))
}
