//! The Downloads screen's filter bar: one chip per [`DownloadFilter`] with
//! its count, the concurrency limit and a settings button. ←/→ move between
//! the chips while the screen shows.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::downloads::{DownloadFilter, MAX_RUNNING};
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme::{self, ChipKind};
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// Below this main-area width the limits text hides, before the chips
/// start to scroll.
const LIMITS_MIN_WIDTH: f32 = 900.0;

impl LuminaApp {
    /// ←/→ on the Downloads screen move between the filter chips. Text
    /// fields and modals take keys before this; other keys go on as before.
    pub(crate) fn handle_downloads_screen_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.downloads_screen_shown() || self.is_fullscreen_viewer || self.search_focused {
            return false;
        }
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt || m.shift || m.function {
            return false;
        }
        match keystroke.key.as_str() {
            "left" => self.move_download_filter(-1, cx),
            "right" => self.move_download_filter(1, cx),
            _ => return false,
        }
        true
    }
}

/// The bar under the top bar: 51.5 high, 28 side padding, bottom border.
/// The chips scroll sideways when the area is narrow.
pub(crate) fn render(app: &LuminaApp, window: &Window, cx: &mut Context<LuminaApp>) -> Div {
    let main_width = crate::views::grid::center_pane_width(
        window.viewport_size().width.into(),
        app.sidebar_width,
        app.sidebar_collapsed,
        false,
        None,
    );
    let selected = app.downloads.filter;
    let chips = DownloadFilter::ALL.into_iter().map(|filter| {
        let key = filter.key();
        let on = filter == selected;
        let label = t(&format!("downloads.filter_{key}"));
        let count = app.download_filter_count(filter);
        theme::chip(
            SharedString::from(format!("downloads-filter-{key}")),
            ChipKind::Normal,
        )
        .debug_selector(move || format!("downloads-filter-{key}"))
        .role(Role::Tab)
        .aria_label(SharedString::from(label.clone()))
        .aria_selected(on)
        .flex_shrink_0()
        .cursor_pointer()
        .text_color(if on { theme::t1() } else { theme::t2() })
        .when(on, |chip| {
            chip.bg(theme::accent_soft()).border_color(theme::accent())
        })
        .when(!on, |chip| chip.hover(|s| s.bg(theme::bg_3())))
        .children(text_marker(
            "downloads-filter-count",
            &format!("{key} {count}").into(),
        ))
        .child(render_icon(
            filter.icon(),
            px(13.0),
            if on { theme::accent() } else { theme::t2() },
        ))
        .child(label)
        .child(
            div()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .text_color(if on { theme::accent() } else { theme::t3() })
                .child(crate::i18n::count(count)),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.set_download_filter(filter, cx);
        }))
    });
    // ponytail: no speed cap until Design 15 adds one; then "· N MB/s cap".
    let limits: SharedString = t_with("downloads.limits", &[&MAX_RUNNING.to_string()]).into();
    let settings_label = t("downloads.settings");
    div()
        .debug_selector(|| "downloads-filter-bar".to_string())
        .w_full()
        .h(px(51.5))
        .flex_shrink_0()
        .px(px(28.0))
        .border_b_1()
        .border_color(theme::line())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .id("downloads-filters")
                .role(Role::TabList)
                .aria_label(SharedString::from(t("downloads.filters")))
                .flex_1()
                .min_w_0()
                .h_full()
                .overflow_x_scroll()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .children(chips),
        )
        .when(main_width >= LIMITS_MIN_WIDTH, |bar| {
            bar.child(
                div()
                    .debug_selector(|| "downloads-limits".to_string())
                    .flex_shrink_0()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(10.5))
                    .text_color(theme::t3())
                    .children(text_marker("downloads-limits", &limits))
                    .child(limits.clone()),
            )
        })
        .child(
            div()
                .id("downloads-settings")
                .debug_selector(|| "downloads-settings".to_string())
                .role(Role::Button)
                .aria_label(SharedString::from(settings_label.clone()))
                .tooltip(theme::tooltip_label(settings_label))
                .size(px(28.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(theme::RADIUS_LG)
                .border_1()
                .border_color(theme::line())
                .bg(theme::bg_2())
                .hover(|s| s.bg(theme::bg_3()))
                .cursor_pointer()
                .child(render_icon(Icon::Settings2, px(14.0), theme::t2()))
                // ponytail: Preferences has no Downloads tab until Design 15;
                // this opens it on its default tab.
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.open_preferences(cx);
                })),
        )
}
