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
            .bg(rgb(0x09090b))
            .text_color(rgb(0xffffff))
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
                                    .bg(rgb(0x3b82f6))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .shadow_xl()
                                    .child(
                                        div()
                                            .text_3xl()
                                            .font_weight(FontWeight::BOLD)
                                            .child("✦"),
                                    ),
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
                                    .text_color(rgb(0xa1a1aa))
                                    .text_base()
                                    .child("Ultra-fast, 120 FPS GPU-accelerated photo & video manager"),
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
                                    .bg(rgb(0x18181b))
                                    .border_1()
                                    .border_color(rgb(0x27272a))
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(rgb(0xd4d4d8))
                                    .child("⚡ 120 FPS ProMotion"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .bg(rgb(0x18181b))
                                    .border_1()
                                    .border_color(rgb(0x27272a))
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(rgb(0xd4d4d8))
                                    .child("🎞 Photos & Videos"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_1p5()
                                    .bg(rgb(0x18181b))
                                    .border_1()
                                    .border_color(rgb(0x27272a))
                                    .rounded_lg()
                                    .text_xs()
                                    .text_color(rgb(0xd4d4d8))
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
                                    .bg(rgb(0x3b82f6))
                                    .hover(|s| s.bg(rgb(0x2563eb)))
                                    .active(|s| s.bg(rgb(0x1d4ed8)))
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
                                    .bg(rgb(0x27272a))
                                    .hover(|s| s.bg(rgb(0x3f3f46)))
                                    .rounded_xl()
                                    .cursor_pointer()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .text_sm()
                                    .text_color(rgb(0xe4e4e7))
                                    .child("📂 Quick Open: Desktop/pics")
                                    .on_click(on_pick_desktop_pics)
                                    .into_any_element()
                            } else {
                                Empty.into_any_element()
                            }),
                    ),
            )
    }
}
