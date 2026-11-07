//! macOS application menu bar.

use gpui::{Menu, MenuItem};

use crate::actions::*;

/// `sidebar_collapsed` picks the View menu's Show/Hide Sidebar title; the
/// app rebuilds the menus when it changes.
pub fn build_menus(sidebar_collapsed: bool) -> Vec<Menu> {
    vec![
        Menu::new("Lumina").items([
            MenuItem::action("About Lumina", ShowAbout),
            MenuItem::separator(),
            MenuItem::action("Preferences…", OpenPreferences),
            MenuItem::separator(),
            MenuItem::action("Quit Lumina", QuitApp),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open Library…", OpenLibrary),
            MenuItem::action("Import…", ImportMedia),
            MenuItem::action("Import Video from URL…", ImportNetworkVideo),
            MenuItem::action("Rescan Library", RescanLibrary),
            MenuItem::separator(),
            MenuItem::action(crate::i18n::t("command.new_album"), NewAlbum),
            MenuItem::action(crate::i18n::t("command.add_to_album"), AddToAlbum),
            MenuItem::action("New Smart Album…", NewSmartAlbum),
            MenuItem::separator(),
            MenuItem::action("Import XMP Metadata", ImportXmp),
            MenuItem::action("Metadata Templates…", ManageTemplates),
        ]),
        Menu::new("View").items([
            MenuItem::action("Command Palette…", ShowCommandPalette),
            MenuItem::separator(),
            MenuItem::action(
                crate::app::panels::sidebar_toggle_title(sidebar_collapsed),
                ToggleSidebar,
            ),
            MenuItem::action("Toggle Inspector", ToggleInspector),
            MenuItem::action("Show Downloads", ToggleDownloads),
            MenuItem::separator(),
            MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
        ]),
        Menu::new("Image").items([
            MenuItem::action("Rotate Left", RotateLeft),
            MenuItem::action("Rotate Right", RotateRight),
            MenuItem::separator(),
            MenuItem::action("Edit Photo", EditPhoto),
            MenuItem::action("Revert to Original", RevertToOriginal),
            MenuItem::separator(),
            MenuItem::action("Edit in External Editor", EditInExternalEditor),
            MenuItem::action("Edit a Copy in External Editor", EditCopyInExternalEditor),
        ]),
        Menu::new("Go").items([
            MenuItem::action("Back", GoBack),
            MenuItem::action("Forward", GoForward),
        ]),
        Menu::new("Help").items([
            MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
            MenuItem::action("About Lumina", ShowAbout),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_has_core_commands() {
        let menus = build_menus(false);
        let names: Vec<&str> = menus.iter().map(|menu| menu.name.as_ref()).collect();
        assert_eq!(names, vec!["Lumina", "File", "View", "Image", "Go", "Help"]);

        let app_items = &menus[0].items;
        assert!(app_items
            .iter()
            .any(|item| matches!(item, MenuItem::Action { name, .. } if name == "Preferences…")));
        assert!(menus[1]
            .items
            .iter()
            .any(|item| matches!(item, MenuItem::Action { name, .. } if name == "Open Library…")));
        assert!(
            menus[1].items.iter().any(
                |item| matches!(item, MenuItem::Action { name, .. } if name == "Metadata Templates…")
            ),
            "File offers the template editor"
        );
    }

    #[test]
    fn go_menu_has_back_and_forward() {
        let menus = build_menus(false);
        let go = menus
            .iter()
            .find(|menu| menu.name.as_ref() == "Go")
            .expect("the menu bar has a Go menu");
        let names: Vec<&str> = go
            .items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Action { name, .. } => Some(name.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["Back", "Forward"], "Go lists Back then Forward");
    }
}
