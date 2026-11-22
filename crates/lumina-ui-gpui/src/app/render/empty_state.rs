//! The shared empty-state shell: glyph, headline, body and up to two actions,
//! used by every grid view and Duplicates when there is nothing to show.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{SmartAlbum, SystemAlbum};

use crate::app::active_view::ViewKey;
use crate::app::rule_label::{rule_icon, rule_label};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::sidebar::AlbumSelection;

mod parts;
pub(crate) use parts::{empty_body, empty_button, empty_glyph, empty_headline, hero_glyph};

/// How large the shell draws: `Hero` for a library with no items at all
/// (First Run style), `Standard` for every other empty view.
#[derive(Clone, Copy)]
enum EmptySize {
    /// 54 px glyph, 34 px buttons.
    Standard,
    /// The 150 px First Run glyph, 40 px buttons.
    Hero,
}

/// Actions offered by a grid empty state. Kept as data so the same button helper
/// can render every state without duplicating click wiring.
#[derive(Clone)]
enum EmptyAction {
    ShowAll,
    /// All Media plus a toast pointing at Add to Album; an empty album's way in.
    AddFromLibrary,
    EditSmartAlbum(SmartAlbum),
    /// Opens the existing delete confirmation; never deletes directly.
    DeleteSmartAlbum(i64),
    OpenLibrary,
    ManageTags,
    Rescan,
    /// Drops one search term (as `tokenize_query` splits them) from the query.
    RemoveSearchTerm(usize),
    ClearSearch,
}

/// What an empty view says and offers.
struct EmptyCopy {
    icon: Icon,
    title: String,
    body: String,
    /// Library folder, shown on its own mono line.
    path: Option<String>,
    /// A smart album's rules, one mono chip each.
    rule_chips: Vec<(Icon, String)>,
    primary: Option<(String, EmptyAction)>,
    secondary: Option<(String, EmptyAction)>,
    /// The library has no items at all: draw at the `Hero` size.
    hero: bool,
}

/// Tab focus on an empty state's buttons, kept with the state it was set
/// in: another view, search or copy (another empty state) has no focus, so
/// a stale index never presses a different action.
pub(crate) struct EmptyFocus {
    view: ViewKey,
    title: String,
    /// Into the shown buttons, primary first.
    index: usize,
}

impl LuminaApp {
    /// Renders an empty view with a clear next action instead of a dead-end line.
    pub(crate) fn render_empty_state(&self, cx: &mut Context<Self>) -> AnyElement {
        let search_terms = lumina_core::search::tokenize_query(&self.search_query);
        let EmptyCopy {
            icon,
            title,
            body,
            path,
            rule_chips,
            primary,
            secondary,
            hero,
        } = self.empty_copy();
        let focus = self.empty_focus_index(&title);
        let size = if hero {
            EmptySize::Hero
        } else {
            EmptySize::Standard
        };
        let secondary_index = usize::from(primary.is_some());
        let mut button = |primary: bool, (label, action): (String, EmptyAction)| {
            let icon = match action {
                EmptyAction::AddFromLibrary => Some(Icon::Library),
                EmptyAction::EditSmartAlbum(_) => Some(Icon::SlidersHorizontal),
                EmptyAction::DeleteSmartAlbum(_) => Some(Icon::Trash),
                EmptyAction::ClearSearch => Some(Icon::X),
                _ => None,
            };
            let index = if primary { 0 } else { secondary_index };
            let id = if primary {
                "empty-primary"
            } else {
                "empty-secondary"
            };
            self.empty_action_button(primary, label, icon, action, size, cx)
                .when(focus == Some(index), |b| {
                    b.border_color(theme::focus_ring())
                        .children(cfg!(test).then(|| {
                            div()
                                .absolute()
                                .size_0()
                                .debug_selector(move || format!("{id}-focused"))
                        }))
                })
        };
        let primary_button = primary.map(|p| button(true, p));
        let secondary_button = secondary.map(|s| button(false, s));
        let glyph = match size {
            EmptySize::Standard => empty_glyph(px(54.0), icon, theme::t2()),
            EmptySize::Hero => hero_glyph(icon),
        };

        div()
            .id("empty-state")
            .debug_selector(|| "empty-state".to_string())
            .flex_1()
            // Shrink to the column between the sidebar and the inspector.
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px(px(28.0))
            .py(px(20.0))
            .child(
                div()
                    .w_full()
                    .max_w(px(430.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(glyph.debug_selector(|| "empty-state-glyph".to_string()))
                    .child(empty_headline("empty-state-headline", title.into()))
                    .child(empty_body("empty-state-body", body.into()))
                    .children(path.map(|path| {
                        div()
                            .mt(px(8.0))
                            .debug_selector(|| "empty-library-path".to_string())
                            .max_w_full()
                            .truncate()
                            .font_family(theme::FONT_SANS)
                            .font_features(theme::tabular_figures())
                            .text_size(theme::TEXT_SMALL)
                            .text_color(theme::t2())
                            .child(path)
                    }))
                    // One chip per rule, the cards' short label and field icon.
                    .when(!rule_chips.is_empty(), |col| {
                        col.child(
                            div()
                                .mt(px(16.0))
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .justify_center()
                                .gap(px(7.0))
                                .children(rule_chips.into_iter().enumerate().map(
                                    |(i, (icon, label))| {
                                        let label = SharedString::from(label);
                                        div()
                                            .debug_selector(move || format!("empty-rule-chip-{i}"))
                                            .h(px(24.0))
                                            .px(px(9.0))
                                            .rounded(px(6.0))
                                            .bg(theme::bg_1())
                                            .border_1()
                                            .border_color(theme::line())
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(6.0))
                                            .font_family(theme::FONT_SANS)
                                            .font_features(theme::tabular_figures())
                                            .text_size(theme::TEXT_CAPTION)
                                            .text_color(theme::t2())
                                            .child(render_icon(
                                                icon,
                                                theme::ICON_XS,
                                                theme::accent(),
                                            ))
                                            .children(cfg!(test).then(|| {
                                                let text = label.clone();
                                                div().absolute().size_0().debug_selector(
                                                    move || {
                                                        format!("empty-rule-chip-{i}-text:{text}")
                                                    },
                                                )
                                            }))
                                            .child(label)
                                    },
                                )),
                        )
                    })
                    // Each search term is a Drop chip; clicking one removes it.
                    .when(!search_terms.is_empty(), |col| {
                        col.child(
                            div()
                                .mt(px(16.0))
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .justify_center()
                                .gap_1p5()
                                .children(search_terms.iter().enumerate().map(|(i, term)| {
                                    // Display only: the term's own quotes would nest in “…”.
                                    let shown = term.replace('"', "");
                                    let label = SharedString::from(
                                        crate::i18n::t("search.empty_drop")
                                            .replace("{term}", &shown),
                                    );
                                    let tooltip = crate::i18n::t("search.empty_drop_tooltip")
                                        .replace("{term}", &shown);
                                    div()
                                        .id(("empty-search-term", i as u64))
                                        .debug_selector(move || format!("empty-search-term-{i}"))
                                        .h(px(28.0))
                                        .px(px(11.0))
                                        .rounded_full()
                                        .bg(theme::accent_soft())
                                        .border_1()
                                        .border_color(theme::accent_outline())
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .text_size(theme::TEXT_SMALL)
                                        .text_color(theme::accent())
                                        .cursor_pointer()
                                        .tooltip(theme::tooltip_label(tooltip))
                                        .role(Role::Button)
                                        .aria_label(label.clone())
                                        .child(render_icon(
                                            Icon::X,
                                            theme::ICON_XS,
                                            theme::accent(),
                                        ))
                                        .children(cfg!(test).then(|| {
                                            let text = label.clone();
                                            div().absolute().size_0().debug_selector(move || {
                                                format!("empty-search-term-{i}-text:{text}")
                                            })
                                        }))
                                        .child(label)
                                        .on_click(cx.listener(
                                            move |this, _e: &ClickEvent, _w, cx| {
                                                this.perform_empty_action(
                                                    EmptyAction::RemoveSearchTerm(i),
                                                    cx,
                                                );
                                            },
                                        ))
                                })),
                        )
                    })
                    .child(
                        div()
                            .mt(px(16.0))
                            .flex()
                            .flex_row()
                            .justify_center()
                            .gap(px(9.0))
                            .children(primary_button)
                            .children(secondary_button),
                    ),
            )
            .into_any_element()
    }

    fn empty_copy(&self) -> EmptyCopy {
        let user_album = match &self.selection {
            AlbumSelection::User(id) => self
                .user_smart_albums
                .iter()
                .find(|album| album.id == *id)
                .cloned(),
            _ => None,
        };

        let search_terms = lumina_core::search::tokenize_query(&self.search_query);
        let show_all = || Some(("Show all photos".to_string(), EmptyAction::ShowAll));
        let copy = |icon, title: String, body: String| EmptyCopy {
            icon,
            title,
            body,
            path: None,
            rule_chips: Vec::new(),
            primary: None,
            secondary: None,
            hero: false,
        };
        // Whitespace-only queries tokenize to nothing; treat them as no search.
        if let Some((title, body)) = self.videos_empty_copy() {
            // The chip row's Clear all (or All clips) is the way back.
            EmptyCopy {
                primary: (!self.videos_4k_active()).then(show_all).flatten(),
                ..copy(Icon::Video, title, body)
            }
        } else if !search_terms.is_empty() {
            // Each chip drops one term; Clear all filters drops them all.
            // ponytail: no per-term counts or kind icons.
            let (title, body) = match search_terms.len() {
                1 => (t("search.empty_title_one"), t("search.empty_body_one")),
                2 => (t("search.empty_title_two"), t("search.empty_body_many")),
                n => {
                    let n = match n {
                        3..=10 => t(&format!("number.{n}")),
                        _ => n.to_string(),
                    };
                    let title = t("search.empty_title_many").replace("{n}", &n);
                    (title, t("search.empty_body_many"))
                }
            };
            EmptyCopy {
                secondary: Some((t("search.empty_clear_all"), EmptyAction::ClearSearch)),
                ..copy(Icon::SearchX, title, body)
            }
        } else {
            match &self.selection {
                AlbumSelection::Trash => {
                    copy(Icon::Trash, t("trash.empty_title"), t("trash.empty_body"))
                }
                // Every group skipped: they come back on the next visit.
                AlbumSelection::Duplicates if !self.duplicate_groups().is_empty() => copy(
                    Icon::StepForward,
                    t("duplicates.all_skipped"),
                    t("duplicates.all_skipped_body"),
                ),
                AlbumSelection::Duplicates => copy(
                    Icon::Repeat,
                    "No Duplicates Found".into(),
                    "Lumina scanned your library and found no duplicate photos or videos.".into(),
                ),
                AlbumSelection::System(SystemAlbum::Favorites) => EmptyCopy {
                    primary: show_all(),
                    ..copy(
                        Icon::Heart,
                        "No favorites yet".into(),
                        "Press F on any photo or video to add it here.".into(),
                    )
                },
                AlbumSelection::User(_) => match user_album {
                    Some(album) => EmptyCopy {
                        rule_chips: album
                            .rules
                            .iter()
                            .map(|rule| (rule_icon(&rule.field), rule_label(rule)))
                            .collect(),
                        primary: Some((
                            crate::i18n::t("smart_album.empty_edit"),
                            EmptyAction::EditSmartAlbum(album.clone()),
                        )),
                        secondary: Some((
                            crate::i18n::t("smart_album.empty_delete"),
                            EmptyAction::DeleteSmartAlbum(album.id),
                        )),
                        ..copy(
                            Icon::Sparkles,
                            crate::i18n::t("smart_album.empty_title"),
                            crate::i18n::t("smart_album.empty_body"),
                        )
                    },
                    None => EmptyCopy {
                        primary: show_all(),
                        ..copy(
                            Icon::Sliders,
                            "Nothing matches this view".into(),
                            "The smart album may have been removed. Show everything instead."
                                .into(),
                        )
                    },
                },
                AlbumSelection::Album(id) => EmptyCopy {
                    // The same condition ⌘K offers the command under.
                    primary: self.empty_album_shown().then(|| {
                        (
                            crate::i18n::t("command.add_from_library"),
                            EmptyAction::AddFromLibrary,
                        )
                    }),
                    ..copy(
                        Icon::ImagePlus,
                        match self.album_title(*id) {
                            Some(title) => {
                                crate::i18n::t("album.empty_title").replace("{title}", title)
                            }
                            None => crate::i18n::t("album.empty_title_unknown"),
                        },
                        crate::i18n::t("album.empty_body"),
                    )
                },
                AlbumSelection::Tag(tag) => EmptyCopy {
                    primary: show_all(),
                    secondary: Some(("Manage tags".into(), EmptyAction::ManageTags)),
                    ..copy(
                        Icon::Tag,
                        "No items found".into(),
                        format!(
                            "Nothing is tagged #{tag} yet. Try another tag or show everything."
                        ),
                    )
                },
                _ => {
                    let root = self
                        .grid_state
                        .as_ref()
                        .map(|state| crate::views::preferences::tilde(state.engine.library_root()));
                    let scanning = self.grid_state.as_ref().is_some_and(|s| s.is_scanning);
                    let choose = Some(("Choose Another Folder…".into(), EmptyAction::OpenLibrary));
                    if let (0, Some(root), true) = (self.total_library_count, &root, scanning) {
                        EmptyCopy {
                            path: Some(root.clone()),
                            primary: choose,
                            hero: true,
                            ..copy(
                                Icon::Folder,
                                "Scanning…".into(),
                                "Photos and videos appear here as they are found.".into(),
                            )
                        }
                    } else if let (0, Some(root)) = (self.total_library_count, root) {
                        EmptyCopy {
                            path: Some(root),
                            primary: choose,
                            secondary: Some(("Rescan".into(), EmptyAction::Rescan)),
                            hero: true,
                            ..copy(
                                Icon::Folder,
                                "No photos or videos found".into(),
                                "Lumina looks for JPEG, HEIC, PNG, TIFF, WebP, RAW and video \
                                 files, including in subfolders."
                                    .into(),
                            )
                        }
                    } else {
                        EmptyCopy {
                            primary: show_all(),
                            ..copy(
                                Icon::Image,
                                "No media in this view".into(),
                                "Try another view or show everything in the library.".into(),
                            )
                        }
                    }
                }
            }
        }
    }

    /// One empty-state action: `empty-primary` (accent) or `empty-secondary`,
    /// with an optional leading icon.
    fn empty_action_button(
        &self,
        primary: bool,
        label: String,
        icon: Option<Icon>,
        action: EmptyAction,
        size: EmptySize,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = if primary {
            "empty-primary"
        } else {
            "empty-secondary"
        };
        let height = match size {
            EmptySize::Hero => px(40.0),
            EmptySize::Standard => px(34.0),
        };
        empty_button(id, primary, label.into(), icon, height).on_click(cx.listener(
            move |this, _event, _window, cx| {
                this.perform_empty_action(action.clone(), cx);
            },
        ))
    }

    /// The Tab-focused button, if the focus was set in the empty state now
    /// shown (titled `title`).
    fn empty_focus_index(&self, title: &str) -> Option<usize> {
        self.active_view
            .empty_focus
            .as_ref()
            .filter(|focus| focus.title == title && focus.view == self.view_key())
            .map(|focus| focus.index)
    }

    /// The grid column draws the empty state (`render_browser_grid`).
    /// Duplicates' empty states have no buttons, so they are left out.
    fn empty_state_shown(&self) -> bool {
        self.grid_state.is_some()
            && !self.is_fullscreen_viewer
            && !self.non_media_page_shown()
            && !self.map_shown()
            && self.selection != AlbumSelection::Duplicates
            && !self.view_loading()
            && self.displayed_count() == 0
    }

    /// The empty state's keys: Tab / ⇧Tab move between its buttons
    /// (wrapping), ↵ presses the focused one and Esc clears the focus. ↵
    /// with none focused, and every other key, fall through.
    pub(crate) fn handle_empty_state_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt || m.function || !self.empty_state_shown() {
            return false;
        }
        let copy = self.empty_copy();
        let focus = self.empty_focus_index(&copy.title);
        let actions: Vec<EmptyAction> = [copy.primary, copy.secondary]
            .into_iter()
            .flatten()
            .map(|(_, action)| action)
            .collect();
        let count = actions.len();
        match (keystroke.key.as_str(), focus) {
            ("tab", _) if count > 0 => {
                let index = match focus {
                    Some(i) if m.shift => (i + count - 1) % count,
                    Some(i) => (i + 1) % count,
                    None if m.shift => count - 1,
                    None => 0,
                };
                self.active_view.empty_focus = Some(EmptyFocus {
                    view: self.view_key(),
                    title: copy.title,
                    index,
                });
                cx.notify();
            }
            ("enter" | "return", Some(index)) if !m.shift => {
                self.active_view.empty_focus = None;
                if let Some(action) = actions.into_iter().nth(index) {
                    self.perform_empty_action(action, cx);
                }
                cx.notify();
            }
            ("escape", Some(_)) => {
                self.active_view.empty_focus = None;
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    fn perform_empty_action(&mut self, action: EmptyAction, cx: &mut Context<Self>) {
        match action {
            EmptyAction::ShowAll => self.select_album(AlbumSelection::AllLibrary, cx),
            EmptyAction::AddFromLibrary => self.add_from_library(cx),
            EmptyAction::EditSmartAlbum(album) => self.open_album_builder_for_edit(album, cx),
            EmptyAction::DeleteSmartAlbum(id) => self.prompt_delete_smart_album(id, cx),
            EmptyAction::OpenLibrary => self.pick_folder_dialog(cx),
            EmptyAction::ManageTags => self.open_tag_manager(cx),
            EmptyAction::Rescan => self.rescan_library(cx),
            EmptyAction::RemoveSearchTerm(index) => {
                let mut rest = lumina_core::search::tokenize_query(&self.search_query);
                if index < rest.len() {
                    rest.remove(index);
                }
                let query = rest.join(" ");
                self.set_search_query(&query, cx);
            }
            EmptyAction::ClearSearch => self.clear_search(cx),
        }
    }

    /// An album with nothing in it is on screen (⌘K Add from Library).
    pub(crate) fn empty_album_shown(&self) -> bool {
        self.album_page_album().is_some() && !self.view_loading() && self.displayed_count() == 0
    }

    /// No picker yet: go to All Media and say how to add items from there.
    pub(crate) fn add_from_library(&mut self, cx: &mut Context<Self>) {
        self.select_album(AlbumSelection::AllLibrary, cx);
        self.show_toast(crate::i18n::t("album.add_from_library_toast"), cx);
    }
}
