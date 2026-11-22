//! The downloads popover's fixed parts around the scrolling rows: the
//! header with its count and actions, the Earlier head with Clear, and the
//! "Show all downloads" footer.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::DownloadState;

use crate::app::preferences::PreferencesTab;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

/// A 25×25 icon button with `label` as its tooltip and VoiceOver name.
pub(crate) fn popover_button(
    selector: String,
    icon: Icon,
    label: String,
    colors: (Rgba, Rgba),
) -> Stateful<Div> {
    named_popover_button(selector, icon, label.clone(), label.into(), colors)
}

/// [`popover_button`] with a VoiceOver name other than its tooltip.
pub(crate) fn named_popover_button(
    selector: String,
    icon: Icon,
    label: String,
    name: SharedString,
    (bg, fg): (Rgba, Rgba),
) -> Stateful<Div> {
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(name)
        .tooltip(theme::tooltip_label(label))
        .size(px(25.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.0))
        .bg(bg)
        .cursor_pointer()
        .hover(|s| s.bg(theme::bg_3()))
        .child(render_icon(icon, px(13.0), fg))
}

/// "Downloads", the "N active" pill, then Pause all (or Resume all when
/// nothing runs and some rows are paused), the Downloads screen and
/// settings on its Downloads category.
pub(crate) fn header(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> Div {
    // Running, waiting and paused rows, as the toolbar badge counts and the
    // Active list shows them.
    let active = app.download_badge_count();
    let any_paused = app.any_download_in(&[DownloadState::Paused]);
    let plain = (theme::bg_1(), theme::t2());
    let toggle = if app.running_or_waiting_count() > 0 {
        Some(
            popover_button(
                "downloads-header-pause".into(),
                Icon::Pause,
                t("downloads.pause_all"),
                plain,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.pause_all_downloads(cx);
            })),
        )
    } else if any_paused {
        Some(
            popover_button(
                "downloads-header-resume".into(),
                Icon::Play,
                t("downloads.resume_all"),
                plain,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.resume_all_downloads(cx);
            })),
        )
    } else {
        None
    };
    div()
        .debug_selector(|| "downloads-header".to_string())
        .flex_shrink_0()
        .h(px(43.5))
        .px(px(13.0))
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        // ponytail: the design's -0.2 tracking is skipped; GPUI has none.
        .child(
            div()
                .font_family(theme::FONT_DISPLAY)
                .text_size(theme::TEXT_BODY)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child(t("downloads.title")),
        )
        .when(active > 0, |el| {
            el.child(
                div()
                    .debug_selector(|| "downloads-header-count".to_string())
                    .h(px(18.0))
                    .px(px(7.0))
                    .rounded_full()
                    .bg(theme::accent_soft())
                    .flex()
                    .items_center()
                    .font_family(theme::FONT_SANS)
                    .font_features(theme::tabular_figures())
                    .text_size(theme::TEXT_CAPTION)
                    .text_color(theme::accent())
                    .child({
                        let text = t_with("downloads.header_active", &[&active.to_string()]);
                        let marker = text.clone();
                        div()
                            .debug_selector(move || format!("downloads-header-count-text:{marker}"))
                            .child(text)
                    }),
            )
        })
        .child(div().flex_1())
        .children(toggle)
        .child(
            popover_button(
                "downloads-header-expand".into(),
                Icon::Maximize,
                t("downloads.show_all"),
                plain,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.show_downloads_screen(cx);
            })),
        )
        .child(
            popover_button(
                "downloads-header-settings".into(),
                Icon::Settings2,
                t("downloads.settings"),
                plain,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.open_preferences_at(PreferencesTab::Downloads, cx);
                // An edit draft refuses settings; then the popover stays.
                if this.preferences_open {
                    this.close_downloads_panel(cx);
                }
            })),
        )
}

/// "EARLIER" with Clear: removes the Earlier rows (finished and already in
/// the library). Failed rows sit in the active list above and stay for Retry.
pub(crate) fn earlier_head(cx: &mut Context<LuminaApp>) -> Div {
    div()
        .debug_selector(|| "downloads-earlier-head".to_string())
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .px(px(10.0))
        .pt(px(10.0))
        .pb(px(4.0))
        .child(
            div()
                .text_size(theme::TEXT_CAPTION)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t3())
                .child(t("downloads.earlier")),
        )
        .child(
            div()
                .id("downloads-clear-finished")
                .debug_selector(|| "downloads-clear-finished".to_string())
                .role(Role::Button)
                .aria_label(t("downloads.clear_completed"))
                .tooltip(theme::tooltip_label(t("downloads.clear_completed")))
                .cursor_pointer()
                .text_size(theme::TEXT_CAPTION)
                .text_color(theme::t3())
                .hover(|s| s.text_color(theme::t1()))
                .child(t("downloads.clear"))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.clear_completed_downloads(cx);
                })),
        )
}

/// "Show all downloads →": the Downloads screen, closing the popover.
pub(crate) fn footer(cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    div()
        .id("downloads-show-all")
        .debug_selector(|| "downloads-show-all".to_string())
        .role(Role::Button)
        .aria_label(t("downloads.show_all"))
        .flex_shrink_0()
        .h(px(39.5))
        .bg(theme::bg_1())
        .border_t_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .cursor_pointer()
        .text_size(theme::TEXT_SMALL)
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::t2())
        .hover(|s| s.bg(theme::bg_2()).text_color(theme::t1()))
        .child(t("downloads.show_all"))
        .child(render_icon(Icon::ArrowRight, px(12.0), theme::t2()))
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.show_downloads_screen(cx);
        }))
}
