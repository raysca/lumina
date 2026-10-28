use crate::icons::{render_icon, Icon};
use crate::theme;
use gpui::*;

pub struct OnboardingView;

impl OnboardingView {
    pub fn render(
        on_pick_folder: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_pick_desktop_pics: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        has_desktop_pics: bool,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .justify_center()
            .items_center()
            .bg(theme::bg_viewer())
            .text_color(theme::text())
            .p_8()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .max_w(px(560.0))
                    .gap_6()
                    // Logo & App Name
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(72.0))
                                    .h(px(72.0))
                                    .rounded_2xl()
                                    .bg(theme::accent())
                                    .text_color(theme::accent_fg())
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .shadow_xl()
                                    .child(render_icon(
                                        Icon::Sparkles,
                                        px(30.0),
                                        theme::accent_fg(),
                                    )),
                            )
                            .child(
                                div()
                                    .text_3xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child("Welcome to Lumina"),
                            )
                            .child(
                                div()
                                    .text_center()
                                    .text_color(theme::text_muted())
                                    .text_base()
                                    .child(
                                        "Ultra-fast, 120 FPS GPU-accelerated photo & video manager",
                                    ),
                            ),
                    )
                    // Feature badges
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_4()
                            .my_2()
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .bg(theme::bg_pane())
                                    .border_1()
                                    .border_color(theme::border())
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1p5()
                                    .child(render_icon(Icon::Zap, px(11.0), theme::text()))
                                    .child("120 FPS ProMotion"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .bg(theme::bg_pane())
                                    .border_1()
                                    .border_color(theme::border())
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child("🎞 Photos & Videos"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .bg(theme::bg_pane())
                                    .border_1()
                                    .border_color(theme::border())
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(theme::text())
                                    .child("🏎 0ms UI Blocking"),
                            ),
                    )
                    // Primary Actions
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .id("pick-folder-btn")
                                    .w_full()
                                    .py_3()
                                    .bg(theme::accent())
                                    .text_color(theme::accent_fg())
                                    .hover(|s| s.bg(theme::accent()).text_color(theme::accent_fg()))
                                    .active(|s| {
                                        s.bg(theme::accent_hover()).text_color(theme::accent_fg())
                                    })
                                    .rounded_xl()
                                    .cursor_pointer()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .text_base()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Choose Photos Folder...")
                                    .on_click(on_pick_folder),
                            )
                            .child(if has_desktop_pics {
                                div()
                                    .id("pick-desktop-pics-btn")
                                    .w_full()
                                    .py_2p5()
                                    .bg(theme::bg_control())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .rounded_xl()
                                    .cursor_pointer()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .text_sm()
                                    .text_color(theme::text())
                                    .child("📂 Quick Open: Pictures")
                                    .on_click(on_pick_desktop_pics)
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    ),
            )
    }
}
