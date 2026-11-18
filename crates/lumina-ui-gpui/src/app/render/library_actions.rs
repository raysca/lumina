//! The library root's menu action handlers.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::actions::*;
use crate::app::LuminaApp;
use crate::views::viewer_state::ViewerRotation;

impl LuminaApp {
    /// The library root's menu actions, each registered only where it can
    /// act on what the user sees.
    pub(super) fn library_actions(
        &self,
        root: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let editing = self.edits.session.is_some();
        // Compare shows (not the viewer opened from it), or a slideshow
        // plays (the menu bar stays reachable in full screen): menu items
        // that act on the hidden selection, open a dialog or move the view
        // are disabled.
        let slideshow = self.slideshow.is_some();
        let presenting = (self.compare.is_some() && !self.is_fullscreen_viewer) || slideshow;
        // The viewer opened from compare, too: these would play, move or
        // trash the view behind compare (the palette leaves them out).
        let holds_view = self.compare_holds_view();
        // Settings is an opaque full-window layer: items that open a dialog
        // under it or act on the hidden library are disabled.
        let settings = self.preferences_open;
        let open_ok = !editing && !presenting && !settings;
        root
            // Compare leaves these on; under a slideshow they would change
            // the panels or open the sheet unseen behind the slide.
            .when(!slideshow, |root| {
                root.on_action(cx.listener(|this, _: &ShowShortcuts, _w, cx| {
                    this.toggle_shortcuts_overlay(cx);
                }))
            })
            // Under settings a toggle would flip and save a setting the user
            // cannot see.
            .when(!slideshow && !settings, |root| {
                root.on_action(cx.listener(|this, _: &ToggleSidebar, _w, cx| {
                    this.toggle_sidebar(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleInspector, _w, cx| {
                    this.toggle_inspector(cx);
                }))
                .when(self.album_panel_fits(), |root| {
                    root.on_action(cx.listener(|this, _: &ToggleAlbumPanel, _w, cx| {
                        this.toggle_album_panel(cx);
                    }))
                })
            })
            // Edit mode: no handler means a disabled menu item. These open
            // overlays that take the keys, change media, or leave the photo
            // under the draft. Rotate and Revert go to the draft; Quit and
            // Open Library stay (they discard the draft and say so).
            .when(!editing && !presenting, |root| {
                // Like ⌘,: Settings… while settings is open closes it.
                root.on_action(cx.listener(|this, _: &OpenPreferences, _w, cx| {
                    if this.preferences_open {
                        this.close_preferences(cx);
                    } else {
                        this.open_preferences(cx);
                    }
                }))
                .on_action(cx.listener(|this, _: &ShowAbout, _w, cx| {
                    this.open_about(cx);
                }))
                // Settings › Library offers Rescan too, so it stays on there.
                .on_action(cx.listener(|this, _: &RescanLibrary, _w, cx| {
                    this.rescan_library(cx);
                }))
                .on_action(cx.listener(
                    |this, _: &ShowCommandPalette, _w, cx| {
                        this.open_command_palette(cx);
                    },
                ))
            })
            .when(open_ok, |root| {
                root.on_action(cx.listener(|this, _: &ImportMedia, _w, cx| {
                    this.pick_import_folders(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportNetworkVideo, _w, cx| {
                    this.open_network_import_dialog(cx);
                }))
                .on_action(cx.listener(|this, _: &ToggleDownloads, _w, cx| {
                    this.toggle_downloads_panel(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowAllDownloads, _w, cx| {
                    this.show_downloads_screen(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportXmp, _w, cx| {
                    this.import_sidecars_selected(cx);
                }))
                .on_action(cx.listener(|this, _: &ManageTemplates, _w, cx| {
                    this.open_template_editor(cx);
                }))
                .on_action(cx.listener(|this, _: &NewAlbum, _w, cx| {
                    this.new_album(cx);
                }))
                .on_action(cx.listener(|this, _: &AddToAlbum, _w, cx| {
                    this.open_album_picker(cx);
                }))
                .on_action(cx.listener(|this, _: &NewSmartAlbum, _w, cx| {
                    this.open_album_builder(cx);
                }))
                .on_action(cx.listener(|this, _: &ExportOriginals, _w, cx| {
                    this.export_pick_folder(lumina_core::ExportPreset::Originals, cx);
                }))
                .on_action(cx.listener(|this, _: &ExportAsJpeg, _w, cx| {
                    this.export_pick_folder(lumina_core::ExportPreset::Jpeg2048, cx);
                }))
                // File > Share… exists on macOS only; elsewhere nothing sends it.
                .on_action(cx.listener(|this, _: &ShareSelection, _w, cx| {
                    #[cfg(target_os = "macos")]
                    this.share_selection(None, cx);
                    #[cfg(not(target_os = "macos"))]
                    let _ = (this, cx);
                }))
                .on_action(cx.listener(|this, _: &Compare, _w, cx| {
                    this.open_compare(cx);
                }))
                .on_action(cx.listener(|this, _: &ImportSmartAlbumRules, _w, cx| {
                    this.import_smart_album_rules(cx);
                }))
                // The editor opens the file without the draft's edits.
                .on_action(cx.listener(|this, _: &EditInExternalEditor, _w, cx| {
                    this.edit_selected_externally(false, cx);
                }))
                .on_action(cx.listener(
                    |this, _: &EditCopyInExternalEditor, _w, cx| {
                        this.edit_selected_externally(true, cx);
                    },
                ))
            })
            .when(open_ok && !holds_view, |root| {
                root.on_action(cx.listener(|this, _: &DeleteRejected, _w, cx| {
                    this.delete_rejected(cx);
                }))
                .on_action(cx.listener(|this, _: &Slideshow, window, cx| {
                    this.start_slideshow(window, cx);
                }))
                .on_action(cx.listener(|this, _: &ShowMap, _w, cx| {
                    this.show_map(cx);
                }))
                .on_action(cx.listener(|this, _: &ShowAlbums, _w, cx| {
                    this.show_albums_overview(cx);
                }))
                .on_action(cx.listener(|this, _: &GoBack, _w, cx| {
                    this.navigate_back(cx);
                }))
                .on_action(cx.listener(|this, _: &GoForward, _w, cx| {
                    this.navigate_forward(cx);
                }))
            })
            .when(!presenting && !settings, |root| {
                root.on_action(cx.listener(|this, _: &RotateLeft, _w, cx| {
                    this.rotate_selection(ViewerRotation::Left, cx);
                }))
                .on_action(cx.listener(|this, _: &RotateRight, _w, cx| {
                    this.rotate_selection(ViewerRotation::Right, cx);
                }))
                .on_action(cx.listener(|this, _: &EditPhoto, _w, cx| {
                    this.enter_edit_mode(cx);
                }))
                .on_action(cx.listener(|this, _: &RevertToOriginal, _w, cx| {
                    this.revert_selection_to_original(cx);
                }))
            })
    }
}
