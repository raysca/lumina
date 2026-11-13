//! The shared empty-state shell: glyph, headline, body and up to two actions,
//! used by every grid view and Duplicates when there is nothing to show.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{SmartAlbum, SystemAlbum};

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::views::sidebar::AlbumSelection;

/// How large the shell draws. `Hero` is for First Run (Design 11); every view
/// today uses `Standard`.
#[derive(Clone, Copy)]
pub(crate) enum EmptySize {
    /// 54 px glyph, 34 px buttons.
    Standard,
    /// 150 px glyph, 40 px primary button.
    // Only the shell test builds it until First Run (Design 11 Task 4) does.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "First Run (Design 11) is the first caller")
    )]
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
    OpenLibrary,
    ManageTags,
    Rescan,
    /// Drops one search term (as `tokenize_query` splits them) from the query.
    RemoveSearchTerm(usize),
}

/// What an empty view says and offers.
struct EmptyCopy {
    icon: Icon,
    title: String,
    body: String,
    /// Library folder, shown on its own mono line.
    path: Option<String>,
    primary: Option<(String, EmptyAction)>,
    secondary: Option<(String, EmptyAction)>,
}

impl LuminaApp {
    /// Renders an empty view with a clear next action instead of a dead-end line.
    pub(crate) fn render_empty_state(&self, size: EmptySize, cx: &mut Context<Self>) -> AnyElement {
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
            primary: None,
            secondary: None,
        };
        // Whitespace-only queries tokenize to nothing; treat them as no search.
        let EmptyCopy {
            icon,
            title,
            body,
            path,
            primary,
            secondary,
        } = if let Some((title, body)) = self.videos_empty_copy() {
            // The chip row's Clear all (or All clips) is the way back.
            EmptyCopy {
                primary: (!self.videos_4k_active()).then(show_all).flatten(),
                ..copy(Icon::Video, title, body)
            }
        } else if !search_terms.is_empty() {
            // The chips are the only actions: removing one widens the search.
            copy(
                Icon::Search,
                "No matches for these filters".into(),
                "Remove a filter to widen the search:".into(),
            )
        } else {
            match &self.selection {
                AlbumSelection::Trash => copy(
                    Icon::Trash,
                    "Recently Deleted is empty".into(),
                    "Items you delete stay here until you empty the trash or restore them.".into(),
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
                        primary: Some((
                            "Edit Rules…".into(),
                            EmptyAction::EditSmartAlbum(album.clone()),
                        )),
                        ..copy(
                            Icon::Sliders,
                            format!("Nothing matches \u{201c}{}\u{201d} yet", album.title),
                            crate::app::smart_draft::rule_summary(album.match_mode, &album.rules),
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
                    primary: Some((
                        crate::i18n::t("command.add_from_library"),
                        EmptyAction::AddFromLibrary,
                    )),
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
                            ..copy(
                                Icon::Folder,
                                "No photos or videos found".into(),
                                "Lumina looks for JPEG, HEIC, PNG, TIFF, WebP, RAW and video \
                                 files, including in subfolders."
                                    .into(),
                            )
                        }
                    } else if self.total_library_count == 0 {
                        EmptyCopy {
                            primary: Some(("Open Library…".into(), EmptyAction::OpenLibrary)),
                            ..copy(
                                Icon::Folder,
                                "Your library is empty".into(),
                                "Open a folder to start building your local photo and video \
                                 library."
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
        };

        let primary_button = primary.map(|(label, action)| {
            let icon = matches!(action, EmptyAction::AddFromLibrary).then_some(Icon::Library);
            self.empty_action_button(true, label, icon, action, size, cx)
        });
        let secondary_button = secondary
            .map(|(label, action)| self.empty_action_button(false, label, None, action, size, cx));
        let glyph = match size {
            EmptySize::Standard => px(54.0),
            EmptySize::Hero => px(150.0),
        };

        div()
            .id("empty-state")
            .debug_selector(|| "empty-state".to_string())
            .flex_1()
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
                    .child(
                        div()
                            .debug_selector(|| "empty-state-glyph".to_string())
                            .size(glyph)
                            .rounded_full()
                            .bg(theme::bg_2())
                            .border_1()
                            .border_color(theme::line())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render_icon(icon, px(20.0), theme::t2())),
                    )
                    .child(
                        div()
                            .debug_selector(|| "empty-state-headline".to_string())
                            .mt(px(14.0))
                            .font_family(theme::FONT_DISPLAY)
                            .text_size(px(18.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .text_center()
                            .child(title),
                    )
                    .child(
                        div()
                            .debug_selector(|| "empty-state-body".to_string())
                            .mt(px(8.0))
                            .text_size(px(12.5))
                            .line_height(px(19.0))
                            .text_color(theme::t2())
                            .text_center()
                            .children(text_marker("empty-state-body", &body.clone().into()))
                            .child(body),
                    )
                    .children(path.map(|path| {
                        div()
                            .mt(px(8.0))
                            .debug_selector(|| "empty-library-path".to_string())
                            .max_w_full()
                            .truncate()
                            .font_family(theme::FONT_MONO)
                            .text_xs()
                            .text_color(theme::t2())
                            .child(path)
                    }))
                    // Each search term is a chip; removing one widens the search.
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
                                    div()
                                        .id(("empty-search-term", i as u64))
                                        .h(px(24.0))
                                        .px(px(10.0))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(theme::line_2())
                                        .bg(theme::bg_3())
                                        .hover(|s| s.bg(theme::bg_3()))
                                        .text_xs()
                                        .text_color(theme::t1())
                                        .cursor_pointer()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .debug_selector(move || format!("empty-search-term-{i}"))
                                        .tooltip(theme::tooltip_label(format!("Remove {term}")))
                                        .font_family(theme::FONT_MONO)
                                        .gap_1p5()
                                        .child(term.clone())
                                        .child(render_icon(Icon::X, px(10.0), theme::t1()))
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
    ) -> AnyElement {
        let id = if primary {
            "empty-primary"
        } else {
            "empty-secondary"
        };
        let (button, ink) = if primary {
            let button = theme::control_primary(id).font_weight(FontWeight::SEMIBOLD);
            (button, theme::on_accent())
        } else {
            (
                theme::control(id).border_color(theme::line_2()),
                theme::t1(),
            )
        };
        let height = match size {
            EmptySize::Hero if primary => px(40.0),
            _ => px(34.0),
        };
        button
            .debug_selector(move || id.to_string())
            .h(height)
            .px(px(14.0))
            .gap(px(7.0))
            .rounded(theme::RADIUS_LG)
            .text_size(px(12.5))
            .children(icon.map(|icon| render_icon(icon, px(14.0), ink)))
            .children(text_marker(id, &label.clone().into()))
            .child(label)
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.perform_empty_action(action.clone(), cx);
            }))
            .into_any_element()
    }

    fn perform_empty_action(&mut self, action: EmptyAction, cx: &mut Context<Self>) {
        match action {
            EmptyAction::ShowAll => self.select_album(AlbumSelection::AllLibrary, cx),
            EmptyAction::AddFromLibrary => self.add_from_library(cx),
            EmptyAction::EditSmartAlbum(album) => self.open_album_builder_for_edit(album, cx),
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
