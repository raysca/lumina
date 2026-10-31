use crate::icons::{render_icon, Icon};
use crate::theme;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Welcome and first-scan column: left-aligned, 520px, 120px from the top.
pub(crate) fn welcome_column() -> Div {
    div()
        .size_full()
        .bg(theme::bg_app())
        .text_color(theme::text())
        .flex()
        .flex_col()
        .items_center()
        .pt(px(120.0))
        .px_8()
}

/// 60px app mark with a 14px radius.
pub(crate) fn logo() -> Div {
    div()
        .size(px(60.0))
        .flex_none()
        .rounded(px(14.0))
        .bg(theme::accent())
        .flex()
        .justify_center()
        .items_center()
        .shadow_lg()
        .child(render_icon(Icon::Sparkles, px(26.0), theme::accent_fg()))
}

pub struct OnboardingView;

impl OnboardingView {
    pub fn render(
        on_pick_folder: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_pick_desktop_pics: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        has_desktop_pics: bool,
    ) -> impl IntoElement {
        welcome_column().child(
            div()
                .id("welcome-column")
                .debug_selector(|| "welcome-column".to_string())
                .w(px(520.0))
                .flex()
                .flex_col()
                .items_start()
                .gap_6()
                .child(logo().debug_selector(|| "welcome-logo".to_string()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_3xl()
                                .font_weight(FontWeight::BOLD)
                                .child("Welcome to Lumina"),
                        )
                        .child(div().text_base().text_color(theme::text_muted()).child(
                            "A fast, private browser for the photos and videos \
                                     already on your Mac. Pick the folder that holds them.",
                        )),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(feature(
                            Icon::Folder,
                            "Works in place",
                            "Your folders stay as they are. Nothing is moved, copied or uploaded.",
                        ))
                        .child(feature(
                            Icon::Film,
                            "Reads everything",
                            "JPEG, HEIC, RAW and video, with dates, cameras and places.",
                        ))
                        .child(feature(
                            Icon::Zap,
                            "Stays fast",
                            "Scrolls smoothly through tens of thousands of items.",
                        )),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .gap_3()
                        .child(
                            button("pick-folder-btn", "Choose Photos Folder…", true)
                                .tooltip(theme::tooltip_label("Choose Photos Folder… (↵)"))
                                .on_click(on_pick_folder),
                        )
                        .children(has_desktop_pics.then(|| {
                            button("pick-desktop-pics-btn", "Use Pictures Folder", false)
                                .tooltip(theme::tooltip_label("Use Pictures Folder (⇧↵)"))
                                .on_click(on_pick_desktop_pics)
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("You can switch to another folder later."),
                ),
        )
    }
}

fn feature(icon: Icon, title: &'static str, body: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .child(
            div()
                .size(px(32.0))
                .flex_none()
                .rounded(theme::RADIUS_LG)
                .bg(theme::bg_raised())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(icon, px(15.0), theme::accent())),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().text_xs().text_color(theme::text_muted()).child(body)),
        )
}

/// Two equal-width buttons: amber primary, quiet secondary.
pub(crate) fn button(id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .flex_1()
        .h(px(38.0))
        .rounded(theme::RADIUS_LG)
        .cursor_pointer()
        .flex()
        .justify_center()
        .items_center()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .map(|b| {
            if primary {
                // Same 1px border as the secondary so flex keeps them equal.
                b.bg(theme::accent())
                    .border_1()
                    .border_color(theme::accent())
                    .text_color(theme::accent_fg())
                    .hover(|s| s.bg(theme::accent_hover()))
            } else {
                b.bg(theme::bg_control())
                    .border_1()
                    .border_color(theme::border())
                    .text_color(theme::text())
                    .hover(|s| s.bg(theme::bg_control_hover()))
            }
        })
        .child(label)
}
