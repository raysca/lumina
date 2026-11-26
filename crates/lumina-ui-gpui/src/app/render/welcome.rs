//! The welcome root shown before any library is open.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::*;
use crate::app::LuminaApp;
use crate::theme;
use crate::views::onboarding::OnboardingView;

use super::app_actions;

impl LuminaApp {
    /// The welcome root, before any library is open: the dormant shell
    /// around the welcome, with settings and the toast above it.
    pub(super) fn render_welcome(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let preferences = self.preferences_layer(cx);
        let library_delete = self.library_delete_layer(cx);
        app_actions(div(), cx)
            // Like ⌘,: Settings… while settings is open closes it.
            .on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                if this.preferences_open {
                    this.close_preferences(cx);
                } else {
                    this.open_preferences(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                this.open_about(cx);
            }))
            .on_action(cx.listener(|this, _: &ImportMedia, _w, cx| {
                this.pick_new_library_for_import(cx);
            }))
            .size_full()
            .font_family(theme::FONT_SANS)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .flex()
            .flex_col()
            .relative()
            .bg(theme::bg_0())
            // The app shell with a dormant sidebar and the
            // All Media header, around the welcome.
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .child(crate::views::sidebar::dormant::render_dormant(self))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            // Settings draws the banner while it covers this.
                            .when(!self.preferences_open, |col| {
                                col.child(self.render_error_banner(cx))
                                    .child(self.render_import_banner(cx))
                            })
                            .child(crate::views::page_header::page_header(
                                None,
                                crate::i18n::t("view.all_media"),
                                Some(crate::i18n::t("header.no_media").into()),
                                Empty,
                                false,
                            ))
                            .child(
                                div()
                                    .id("welcome-body")
                                    .debug_selector(|| "welcome-body".to_string())
                                    .flex_1()
                                    .min_h_0()
                                    // A small window scrolls the welcome.
                                    .overflow_y_scroll()
                                    .track_scroll(&self.welcome.body_scroll)
                                    // While a library opens, show what folder is opening instead
                                    // of flashing OnboardingView or leaving the body blank.
                                    .child(match self.welcome.opening_path() {
                                        Some(path) => crate::views::onboarding::welcome_column()
                                            .child(
                                                div()
                                                    .id("welcome-opening")
                                                    .debug_selector(|| {
                                                        "welcome-opening".to_string()
                                                    })
                                                    .w(px(520.0))
                                                    .flex()
                                                    .flex_col()
                                                    .items_start()
                                                    .gap_3()
                                                    .child(crate::views::onboarding::logo())
                                                    .child(
                                                        div()
                                                            .text_size(theme::TEXT_DISPLAY)
                                                            .font_weight(FontWeight::SEMIBOLD)
                                                            .child(format!(
                                                                "Opening {}…",
                                                                crate::app::recents::library_name(
                                                                    path
                                                                )
                                                            )),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(theme::TEXT_BODY)
                                                            .text_color(theme::t2())
                                                            .child(path.display().to_string()),
                                                    ),
                                            )
                                            .into_any_element(),
                                        None => OnboardingView::render(self.welcome_props(cx)),
                                    }),
                            ),
                    ),
            )
            .child(preferences)
            // A recent library can be removed before any library is opened.
            .child(library_delete)
            // Above settings, which is opaque.
            .child(self.render_toast_overlay(cx))
            .into_any_element()
    }
}
