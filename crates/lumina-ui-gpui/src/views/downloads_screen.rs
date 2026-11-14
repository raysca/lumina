//! The Downloads screen (frame `ttuOX`): the download list in the main area,
//! under a top bar with Pause all / Resume all, Clear completed and Add URL.
//! The body is a plain list for now; filter chips, the status bar and
//! sections come later.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::DownloadState;

use crate::app::render::empty_state::{empty_body, empty_button, empty_glyph, empty_headline};
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::{header_primary_button, text_marker};
use crate::views::sidebar::AlbumSelection;
use crate::LuminaApp;

impl LuminaApp {
    /// The Downloads screen is what the main column shows: selected, with
    /// no search or year over it (those show their matches instead).
    pub(crate) fn downloads_screen_shown(&self) -> bool {
        self.selection == AlbumSelection::Downloads
            && self.search_query.is_empty()
            && self.active_year.is_none()
    }

    /// The sidebar Downloads row and ⌘K / View > Show All Downloads. Works
    /// with an empty queue (the screen says so). The dropdown closes: the
    /// screen shows the same rows.
    pub fn show_downloads_screen(&mut self, cx: &mut Context<Self>) {
        if self.grid_state.is_none() || self.compare_holds_view() {
            return;
        }
        if self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        self.close_downloads_panel(cx);
        if !self.downloads_screen_shown() {
            self.select_album(AlbumSelection::Downloads, cx);
        }
    }

    /// The top bar's meta line, e.g. "2 downloading · 1 paused · 1 queued ·
    /// 1 failed · 24.8 MB/s". Zero parts are left out, and the speed until
    /// one is known; with only finished rows it reads "N completed".
    pub(crate) fn downloads_meta(&self) -> Option<String> {
        let count = |states: &[DownloadState]| {
            self.downloads
                .entries
                .iter()
                .filter(|entry| states.contains(&entry.row.state))
                .count()
        };
        let speed: u64 = self
            .downloads
            .entries
            .iter()
            .filter(|entry| entry.row.state == DownloadState::Running)
            .filter_map(|entry| entry.progress.as_ref())
            .map(|progress| progress.speed_bytes_per_sec)
            .sum();
        let completed: &[DownloadState] = &[DownloadState::Done, DownloadState::Duplicate];
        let mut parts: Vec<String> = [
            (&[DownloadState::Running][..], "downloads.meta_downloading"),
            (&[DownloadState::Paused][..], "downloads.meta_paused"),
            (&[DownloadState::Waiting][..], "downloads.meta_queued"),
            (&[DownloadState::Failed][..], "downloads.meta_failed"),
        ]
        .into_iter()
        .filter_map(|(states, key)| {
            let n = count(states);
            (n > 0).then(|| t_with(key, &[&crate::i18n::count(n)]))
        })
        .collect();
        // Completed rows count only when nothing is in progress (failures
        // aside), so the line leads with what still needs time.
        let done = count(completed);
        if done > 0 && parts.len() == usize::from(count(&[DownloadState::Failed]) > 0) {
            parts.push(t_with(
                "downloads.meta_completed",
                &[&crate::i18n::count(done)],
            ));
        }
        if speed > 0 {
            parts.push(format!("{}/s", format_bytes(speed)));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

/// The top bar's actions. Pause all and Clear completed stay in place,
/// dimmed and inert (no click handler), when there is nothing to act on, so
/// the bar doesn't jump.
pub(crate) fn header_actions(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Div {
    let has = |states: &[DownloadState]| app.any_download_in(states);
    let resume = !has(&[DownloadState::Running]) && has(&[DownloadState::Paused]);
    let can_pause = app.downloads.entries.iter().any(|e| {
        e.row.id > 0 && matches!(e.row.state, DownloadState::Running | DownloadState::Waiting)
    });
    let (icon, label) = if resume {
        (Icon::Play, t("downloads.resume_all"))
    } else {
        (Icon::Pause, t("downloads.pause_all"))
    };
    let pause = text_button(
        "downloads-pause-all",
        icon,
        label,
        (!(resume || can_pause)).then(|| t("downloads.nothing_to_pause")),
    )
    .when(resume || can_pause, |button| {
        button.on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            if resume {
                this.resume_all_downloads(cx);
            } else {
                this.pause_all_downloads(cx);
            }
        }))
    });
    let completed = has(&[DownloadState::Done, DownloadState::Duplicate]);
    let clear = text_button(
        "downloads-clear-completed",
        Icon::CheckCheck,
        t("downloads.clear_completed"),
        (!completed).then(|| t("downloads.nothing_to_clear")),
    )
    .when(completed, |button| {
        button.on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.clear_completed_downloads(cx);
        }))
    });
    let add = header_primary_button(
        "downloads-add-url",
        Icon::Link,
        t("downloads.add_url"),
        t("downloads.add_url_tooltip"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.open_network_import_dialog(cx);
    }));
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .child(pause)
        .child(clear)
        .child(add)
}

/// 32 high, bg-2, 8px radius, `line` outline, icon 14 and a 12.5 t1 label.
/// `disabled` (why there is nothing to do) dims it and is read out as its
/// description; callers attach a click only when enabled.
fn text_button(
    selector: &'static str,
    icon: Icon,
    label: String,
    disabled: Option<String>,
) -> Stateful<Div> {
    let label = SharedString::from(label);
    let enabled = disabled.is_none();
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .h(px(32.0))
        .px(px(12.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(7.0))
        .rounded(theme::RADIUS_LG)
        .border_1()
        .border_color(theme::line())
        .bg(theme::bg_2())
        .text_size(px(12.5))
        .text_color(theme::t1())
        .when(enabled, |b| {
            b.hover(|s| s.bg(theme::bg_3())).cursor_pointer()
        })
        .when_some(disabled, |b, why| b.opacity(0.5).aria_description(why))
        .children(text_marker(selector, &label))
        .child(render_icon(icon, px(14.0), theme::t1()))
        .child(label)
}

/// The screen body: the empty state, or one plain line per row.
pub(crate) fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let body = div()
        .id("downloads-screen")
        .debug_selector(|| "downloads-screen".to_string())
        .size_full()
        .bg(theme::bg_0());
    if app.downloads.entries.is_empty() {
        return body
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(empty_glyph(px(54.0), Icon::ArrowDownToLine, theme::t2()))
            .child(empty_headline(
                "downloads-empty-title",
                t("downloads.empty_title").into(),
            ))
            .child(empty_body(
                "downloads-empty-body",
                t("downloads.empty_body").into(),
            ))
            .child(
                div().mt(px(18.0)).child(
                    empty_button(
                        "downloads-empty-add-url",
                        true,
                        t("downloads.add_url").into(),
                        Some(Icon::Link),
                        px(34.0),
                    )
                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.open_network_import_dialog(cx);
                    })),
                ),
            );
    }
    // ponytail: a plain, display-only list until the sections land (Task 5);
    // it does nothing on click, so no key or click acts on a hidden row.
    body.overflow_y_scroll()
        .px(px(28.0))
        .py(px(18.0))
        .flex()
        .flex_col()
        .gap(px(6.0))
        .children(app.download_display_order().into_iter().map(|entry| {
            let row = &entry.row;
            let title = row.title.clone().unwrap_or_else(|| row.url.clone());
            let id = row.id;
            div()
                .debug_selector(move || format!("downloads-screen-row-{id}"))
                .h(px(40.0))
                .px(px(14.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .rounded(theme::RADIUS_LG)
                .bg(theme::bg_1())
                .border_1()
                .border_color(theme::line())
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.0))
                        .text_color(theme::t1())
                        .child(title),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .font_family(theme::FONT_MONO)
                        .text_size(px(11.0))
                        .text_color(theme::t3())
                        .child(state_word(row.state)),
                )
        }))
}

fn state_word(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Running => "Downloading",
        DownloadState::Waiting => "Queued",
        DownloadState::Paused => "Paused",
        DownloadState::Failed => "Failed",
        DownloadState::Done | DownloadState::Duplicate => "Completed",
    }
}
