use crate::app::render::empty_state::{empty_body, empty_button, empty_headline, hero_glyph};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::missing_library::{missing_library_state, MissingLibraryProps};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Welcome and first-scan column: centred, 120px from the top.
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
        let choose = t("welcome.choose_folder");
        let import = t("welcome.import_new");
        welcome_column()
            .child(
                div()
                    .id("welcome-column")
                    .debug_selector(|| "welcome-column".to_string())
                    .w_full()
                    .max_w(px(640.0))
                    .pb(px(48.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        hero_glyph(Icon::Aperture).debug_selector(|| "welcome-glyph".to_string()),
                    )
                    .child(
                        // ponytail: the design's -0.6 tracking is skipped; GPUI
                        // 0.3.11 has no letter-spacing style.
                        empty_headline("welcome-title", t("welcome.empty_title").into())
                            .mt(px(20.0))
                            .text_size(px(28.0)),
                    )
                    .child(
                        empty_body("welcome-intro", t("welcome.empty_body").into())
                            .mt(px(12.0))
                            .max_w(px(470.0))
                            .text_size(px(13.5))
                            .line_height(px(21.0)),
                    )
                    .child(
                        div()
                            .mt(px(26.0))
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .justify_center()
                            .gap(px(10.0))
                            .child(
                                welcome_button("welcome-open-btn", true, &choose, Icon::FolderOpen)
                                    .child(shortcut_hint("welcome-open-key", "↵"))
                                    .tooltip(theme::tooltip_label(format!("{choose} (↵)")))
                                    .on_click(on_open),
                            )
                            .child(
                                welcome_button(
                                    "welcome-import-new-btn",
                                    false,
                                    &import,
                                    Icon::Folder,
                                )
                                .child(shortcut_hint("welcome-import-new-key", "⇧⌘I"))
                                .tooltip(theme::tooltip_label(format!("{import} (⇧⌘I)")))
                                .on_click(on_import_new),
                            ),
                    )
                    .child(
                        div()
                            .mt(px(14.0))
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
                                sample_busy,
                                on_sample,
                            ))
                            .children(has_desktop_pics.then(|| {
                                text_link(
                                    "welcome-pictures-link",
                                    Role::Link,
                                    t("welcome.use_pictures"),
                                    ("welcome-pictures-key", "⇧↵"),
                                    false,
                                    on_pick_desktop_pics,
                                )
                            })),
                    )
                    .child(
                        div()
                            .mt(px(46.0))
                            .w_full()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap(px(12.0))
                            .child(hint_card(
                                0,
                                Icon::FolderSync,
                                "welcome.hint_watch_title",
                                "welcome.hint_watch_body",
                            ))
                            .child(hint_card(
                                1,
                                Icon::FileImage,
                                "welcome.hint_formats_title",
                                "welcome.hint_formats_body",
                            ))
                            .child(hint_card(
                                2,
                                Icon::ShieldCheck,
                                "welcome.hint_device_title",
                                "welcome.hint_device_body",
                            )),
                    )
                    .when(!recents.is_empty(), |column| {
                        column.child(recent_libraries(recents))
                    }),
            )
            .into_any_element()
    }
}

/// A First Run button: 40 high, 18 px padding, 9 px radius, 15 px icon, 13 px
/// label. Callers add the shortcut hint, tooltip and click.
fn welcome_button(id: &'static str, primary: bool, label: &str, icon: Icon) -> Stateful<Div> {
    empty_button(id, primary, label.to_string().into(), Some(icon), px(40.0))
        .px(px(18.0))
        .gap(px(8.0))
        .rounded(px(9.0))
        .text_size(px(13.0))
}

/// One of the three equal hint cards under the links (`welcome-hint-{i}`).
/// They wrap onto more rows in a narrow window.
fn hint_card(i: usize, icon: Icon, title_key: &str, body_key: &str) -> Div {
    div()
        .debug_selector(move || format!("welcome-hint-{i}"))
        .flex_1()
        .min_w(px(180.0))
        .p(px(14.0))
        .flex()
        .flex_col()
        .gap(px(7.0))
        .rounded(px(9.0))
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line())
        .child(render_icon(icon, px(16.0), theme::accent()))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::t1())
                .child(t(title_key)),
        )
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(theme::t3())
                .child(t(body_key)),
        )
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
    busy: bool,
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
        .text_size(px(12.0))
        .map(|link| {
            // Busy (the sample is being written): dimmed, and a click does nothing.
            if busy {
                link.text_color(theme::t3())
                    .aria_description(t("welcome.sample_busy"))
            } else {
                link.text_color(theme::accent())
                    .cursor_pointer()
                    .hover(|s| s.underline())
                    .on_click(on_click)
            }
        })
        .child(label)
        .child(shortcut_hint(hint_id, keys))
}

/// RECENT LIBRARIES: ↑/↓ focus, ↵ or a click opens, ⌫ or × forgets. A
/// 520 px list (the welcome body scrolls, so a long list needs no scroll of
/// its own).
fn recent_libraries(rows: Vec<RecentRow>) -> impl IntoElement {
    div()
        .debug_selector(|| "welcome-recents".to_string())
        .mt(px(32.0))
        .w(px(520.0))
        .max_w_full()
        .flex()
        .flex_col()
        .gap(px(3.0))
        // The sidebar's section label, 5 px over the rows like there.
        .child(
            div()
                .px(px(10.0))
                .pb(px(2.0))
                .child(theme::section_label(t("welcome.recent_heading"))),
        )
        .children(
            rows.into_iter()
                .enumerate()
                .map(|(i, row)| recent_row(i, row)),
        )
}

/// One recent: a 32 px row (sidebar row tokens) with the name in t1 13 and
/// the path in mono 10 t3, cut in the middle; its tooltip has the full path.
fn recent_row(i: usize, row: RecentRow) -> impl IntoElement {
    let selector = format!("welcome-recent-{i}");
    let path_selector = format!("welcome-recent-path-{i}");
    let remove_selector = format!("welcome-recent-remove-{i}");
    let remove_label = t("welcome.remove_recent_named").replace("{name}", &row.name);
    let on_remove = row.on_remove;
    let mut open_label = t("welcome.open_recent").replace("{name}", &row.name);
    if row.unavailable {
        // Dimming alone says nothing to VoiceOver.
        open_label = format!("{open_label}, {}", t("welcome.not_available"));
    }
    let path = SharedString::from(row.path);
    div()
        .id(("welcome-recent", i))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(open_label)
        .w_full()
        .h(px(32.0))
        .px(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .rounded(theme::RADIUS_LG)
        // Always a border, so the focus ring never shifts the row.
        .border_1()
        .cursor_pointer()
        .map(|r| {
            if row.focused {
                r.bg(theme::bg_3()).border_color(theme::focus_ring())
            } else {
                r.border_color(gpui::transparent_black())
                    .hover(|s| s.bg(theme::bg_2()))
            }
        })
        .on_click(row.on_open)
        .child(
            render_icon(
                Icon::Folder,
                px(16.0),
                if row.unavailable {
                    theme::t3()
                } else {
                    theme::t2()
                },
            )
            .flex_none(),
        )
        .child(
            div()
                .flex_shrink_0()
                .max_w(px(200.0))
                .truncate()
                .text_size(theme::TEXT_BODY)
                .text_color(if row.unavailable {
                    theme::t3()
                } else {
                    theme::t1()
                })
                .child(row.name),
        )
        .child(
            div()
                .id(("welcome-recent-path", i))
                .debug_selector(move || path_selector.clone())
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_middle()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.0))
                .text_color(theme::t3())
                .tooltip(theme::tooltip_label(path.clone()))
                .children(crate::views::page_header::text_marker(
                    "welcome-recent-path",
                    &path,
                ))
                .child(path),
        )
        .when(row.unavailable, |r| {
            let unavailable_selector = format!("welcome-recent-unavailable-{i}");
            r.child(
                div()
                    .debug_selector(move || unavailable_selector.clone())
                    .flex_none()
                    .text_size(px(11.0))
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
