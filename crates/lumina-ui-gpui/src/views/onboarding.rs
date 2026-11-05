use crate::i18n::t;
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

/// A click handler for a welcome control.
pub type WelcomeHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One Recent Libraries row.
pub struct RecentRow {
    pub name: String,
    pub path: String,
    /// The folder was not there when checked; the row says so in text.
    pub unavailable: bool,
    /// Focused from the keyboard (↑/↓).
    pub focused: bool,
    pub on_open: WelcomeHandler,
    pub on_remove: WelcomeHandler,
}

/// What the welcome screen shows (AGENTS §5.1: a struct, not a long
/// parameter list).
pub struct WelcomeProps {
    pub on_pick_folder: WelcomeHandler,
    pub on_pick_desktop_pics: WelcomeHandler,
    pub has_desktop_pics: bool,
    pub recents: Vec<RecentRow>,
    /// The notice for a library that is not where it was.
    pub missing_notice: Option<String>,
}

pub struct OnboardingView;

impl OnboardingView {
    pub fn render(props: WelcomeProps) -> impl IntoElement {
        let WelcomeProps {
            on_pick_folder,
            on_pick_desktop_pics,
            has_desktop_pics,
            recents,
            missing_notice,
        } = props;
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
                .children(missing_notice.map(missing_library_notice))
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
                .when(!recents.is_empty(), |column| {
                    column.child(recent_libraries(recents))
                })
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

/// The last or chosen library is not where it was.
fn missing_library_notice(message: String) -> impl IntoElement {
    div()
        .id("welcome-missing-library")
        .debug_selector(|| "welcome-missing-library".to_string())
        .role(Role::Alert)
        .aria_label(message.clone())
        .w_full()
        .flex()
        .flex_row()
        .items_start()
        .gap_3()
        .p_3()
        .rounded(theme::RADIUS_LG)
        .bg(theme::danger_bg())
        .border_1()
        .border_color(theme::danger_border())
        .child(render_icon(
            Icon::TriangleAlert,
            px(15.0),
            theme::danger_text(),
        ))
        .child(div().text_sm().text_color(theme::text()).child(message))
}

/// RECENT LIBRARIES: ↑/↓ focus, ↵ or a click opens, ⌫ or × forgets.
fn recent_libraries(rows: Vec<RecentRow>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .child(theme::section_label(t("welcome.recent_heading")).mb_1())
        .children(
            rows.into_iter()
                .enumerate()
                .map(|(i, row)| recent_row(i, row)),
        )
}

fn recent_row(i: usize, row: RecentRow) -> impl IntoElement {
    let selector = format!("welcome-recent-{i}");
    let remove_selector = format!("welcome-recent-remove-{i}");
    let remove_label = t("welcome.remove_recent_named").replace("{name}", &row.name);
    let on_remove = row.on_remove;
    let mut open_label = t("welcome.open_recent").replace("{name}", &row.name);
    if row.unavailable {
        // Dimming alone says nothing to VoiceOver.
        open_label = format!("{open_label}, {}", t("welcome.not_available"));
    }
    div()
        .id(("welcome-recent", i))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(open_label)
        .w_full()
        .h(px(44.0))
        .px_3()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .rounded(theme::RADIUS_LG)
        .border_1()
        .cursor_pointer()
        .map(|r| {
            if row.focused {
                r.bg(theme::selection_bg())
                    .border_color(theme::focus_ring())
            } else {
                r.border_color(theme::border_subtle())
                    .hover(|s| s.bg(theme::bg_control_hover()))
            }
        })
        .on_click(row.on_open)
        .child(render_icon(
            Icon::Folder,
            px(15.0),
            if row.unavailable {
                theme::text_disabled()
            } else {
                theme::text_muted()
            },
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if row.unavailable {
                            theme::text_disabled()
                        } else {
                            theme::text()
                        })
                        .truncate()
                        .child(row.name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .truncate()
                        .child(row.path),
                ),
        )
        .when(row.unavailable, |r| {
            let unavailable_selector = format!("welcome-recent-unavailable-{i}");
            r.child(
                div()
                    .debug_selector(move || unavailable_selector.clone())
                    .flex_none()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(t("welcome.not_available")),
            )
        })
        .child(
            div()
                .id(("welcome-recent-remove", i))
                .debug_selector(move || remove_selector.clone())
                .role(Role::Button)
                .aria_label(remove_label)
                .flex_none()
                .size(px(24.0))
                .rounded(theme::RADIUS_MD)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(theme::bg_control_hover()))
                .tooltip(theme::tooltip_label(format!(
                    "{} (⌫)",
                    t("welcome.remove_recent")
                )))
                .on_click(move |event, window, cx| {
                    // The row behind it would open the library.
                    cx.stop_propagation();
                    on_remove(event, window, cx);
                })
                .child(render_icon(Icon::X, px(13.0), theme::text_muted())),
        )
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
                .child(render_icon(icon, px(15.0), theme::text_muted())),
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
