//! macOS application menu bar.

use gpui::{Menu, MenuItem};

use crate::actions::*;

pub fn build_menus() -> Vec<Menu> {
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
            MenuItem::action("New Smart Album…", NewSmartAlbum),
            MenuItem::separator(),
            MenuItem::action("Import XMP Metadata", ImportXmp),
            MenuItem::action("Metadata Templates…", ManageTemplates),
        ]),
        Menu::new("View").items([
            MenuItem::action("Command Palette…", ShowCommandPalette),
            MenuItem::separator(),
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle Inspector", ToggleInspector),
            MenuItem::action("Show Downloads", ToggleDownloads),
            MenuItem::separator(),
            MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
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
        let menus = build_menus();
        let names: Vec<&str> = menus.iter().map(|menu| menu.name.as_ref()).collect();
        assert_eq!(names, vec!["Lumina", "File", "View", "Go", "Help"]);

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
        let menus = build_menus();
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
