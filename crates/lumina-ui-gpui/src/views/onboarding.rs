use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::missing_library::{missing_library_state, MissingLibraryProps};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Welcome and first-scan column: left-aligned, 520px, 120px from the top.
pub(crate) fn welcome_column() -> Div {
    div()
        // At least the window's height, and taller when the content is, so
        // the welcome body's scroll reaches the bottom.
        .w_full()
        .min_h_full()
        .bg(theme::bg_0())
        .text_color(theme::t1())
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
        .child(render_icon(Icon::Sparkles, px(26.0), theme::on_accent()))
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
    /// Explore a Sample Library (⌥↵).
    pub on_sample: WelcomeHandler,
    /// The sample photos are being written.
    pub sample_busy: bool,
    pub recents: Vec<RecentRow>,
    /// The library that is not where it was; replaces the welcome content.
    pub missing: Option<MissingLibraryProps>,
}

pub struct OnboardingView;

impl OnboardingView {
    pub fn render(props: WelcomeProps) -> AnyElement {
        if let Some(missing) = props.missing {
            return missing_library_state(missing);
        }
        let WelcomeProps {
            on_open,
            on_import_new,
            on_pick_desktop_pics,
            has_desktop_pics,
            on_sample,
            sample_busy,
            recents,
            missing: _,
        } = props;
        welcome_column()
            .child(
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
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(t("welcome.title")),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .text_color(theme::t2())
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
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_6()
                            .child(text_link(
                                "welcome-sample-btn",
                                Role::Button,
                                if sample_busy {
                                    t("welcome.sample_preparing")
                                } else {
                                    t("welcome.sample")
                                },
                                ("welcome-sample-key", "⌥↵"),
                                on_sample,
                            ))
                            .children(has_desktop_pics.then(|| {
                                text_link(
                                    "welcome-pictures-link",
                                    Role::Link,
                                    t("welcome.use_pictures"),
                                    ("welcome-pictures-key", "⇧↵"),
                                    on_pick_desktop_pics,
                                )
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::t2())
                            .child(t("welcome.footer")),
                    ),
            )
            .into_any_element()
    }
}

/// The shortcut shown next to a welcome control's label.
fn shortcut_hint(selector: &'static str, keys: &'static str) -> Div {
    theme::kbd(keys)
        .debug_selector(move || selector.to_string())
        .flex_none()
}

/// A quiet secondary path under the two buttons ("Explore a Sample
/// Library", "Use Pictures Folder"), with its shortcut shown beside it.
fn text_link(
    id: &'static str,
    role: Role,
    label: String,
    (hint_id, keys): (&'static str, &'static str),
    on_click: WelcomeHandler,
) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(role)
        .aria_label(format!("{label} ({keys})"))
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
        .child(shortcut_hint(hint_id, keys))
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
                r.bg(theme::bg_3()).border_color(theme::focus_ring())
            } else {
                r.border_color(theme::line()).hover(|s| s.bg(theme::bg_3()))
            }
        })
        .on_click(row.on_open)
        .child(render_icon(
            Icon::Folder,
            px(15.0),
            if row.unavailable {
                theme::t3()
            } else {
                theme::t2()
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
                            theme::t3()
                        } else {
                            theme::t1()
                        })
                        .truncate()
                        .child(row.name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::t2())
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
                    .text_color(theme::t2())
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
                .hover(|s| s.bg(theme::bg_3()))
                .tooltip(theme::tooltip_label(format!(
                    "{} (⌫)",
                    t("welcome.remove_recent")
                )))
                .on_click(move |event, window, cx| {
                    // The row behind it would open the library.
                    cx.stop_propagation();
                    on_remove(event, window, cx);
                })
                .child(render_icon(Icon::X, px(13.0), theme::t2())),
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
                .bg(theme::bg_2())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(icon, px(15.0), theme::t2())),
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
                .child(div().text_xs().text_color(theme::t2()).child(body)),
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
                    .text_color(theme::on_accent())
                    .hover(|s| s.bg(theme::accent_hover()))
            } else {
                b.bg(theme::bg_2())
                    .border_1()
                    .border_color(theme::line())
                    .text_color(theme::t1())
                    .hover(|s| s.bg(theme::bg_3()))
            }
        })
        .child(label)
}
