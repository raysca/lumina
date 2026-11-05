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
    /// Open Existing Library… (↵).
    pub on_open: WelcomeHandler,
    /// Import into New Library… (⇧⌘I).
    pub on_import_new: WelcomeHandler,
    /// Use Pictures Folder (⇧↵), shown only when there is one.
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
            on_open,
            on_import_new,
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
                                .child(t("welcome.title")),
                        )
                        .child(
                            div()
                                .text_base()
                                .text_color(theme::text_muted())
                                .child(t("welcome.intro")),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(feature(
                            Icon::Folder,
                            t("welcome.in_place_title"),
                            t("welcome.in_place_body"),
                        ))
                        .child(feature(
                            Icon::Check,
                            t("welcome.originals_title"),
                            t("welcome.originals_body"),
                        ))
                        .child(feature(
                            Icon::Database,
                            t("welcome.catalog_title"),
                            t("welcome.catalog_body"),
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
                            button("welcome-open-btn", t("welcome.open_existing"), true)
                                .child(shortcut_hint("welcome-open-key", "↵"))
                                .tooltip(theme::tooltip_label(format!(
                                    "{} (↵)",
                                    t("welcome.open_existing")
                                )))
                                .on_click(on_open),
                        )
                        .child(
                            button("welcome-import-new-btn", t("welcome.import_new"), false)
                                .child(shortcut_hint("welcome-import-new-key", "⇧⌘I"))
                                .tooltip(theme::tooltip_label(format!(
                                    "{} (⇧⌘I)",
                                    t("welcome.import_new")
                                )))
                                .on_click(on_import_new),
                        ),
                )
                .children(has_desktop_pics.then(|| pictures_link(on_pick_desktop_pics)))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child(t("welcome.footer")),
                ),
        )
    }
}

/// The shortcut shown next to a welcome control's label.
fn shortcut_hint(selector: &'static str, keys: &'static str) -> Div {
    theme::kbd(keys)
        .debug_selector(move || selector.to_string())
        .flex_none()
}

/// "Use Pictures Folder": a quiet secondary link under the two buttons.
fn pictures_link(on_click: WelcomeHandler) -> impl IntoElement {
    let label = t("welcome.use_pictures");
    div()
        .id("welcome-pictures-link")
        .debug_selector(|| "welcome-pictures-link".to_string())
        .role(Role::Link)
        .aria_label(format!("{label} (⇧↵)"))
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .cursor_pointer()
        .text_sm()
        .text_color(theme::accent())
        .hover(|s| s.underline())
        .on_click(on_click)
        .child(label)
        .child(shortcut_hint("welcome-pictures-key", "⇧↵"))
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

fn feature(icon: Icon, title: String, body: String) -> impl IntoElement {
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

/// Two equal-width buttons: amber primary, quiet secondary. Callers may add
/// a shortcut hint as a further child.
pub(crate) fn button(
    id: &'static str,
    label: impl Into<SharedString>,
    primary: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .flex_1()
        .h(px(38.0))
        .rounded(theme::RADIUS_LG)
        .cursor_pointer()
        .flex()
        .justify_center()
        .items_center()
        .gap_2()
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
