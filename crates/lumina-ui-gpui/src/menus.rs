//! macOS application menu bar.

use gpui::{Menu, MenuItem};

use crate::actions::*;

pub fn build_menus() -> Vec<Menu> {
    vec![
        Menu {
            name: "Lumina".into(),
            items: vec![
                MenuItem::action("About Lumina", ShowAbout),
                MenuItem::separator(),
                MenuItem::action("Preferences…", OpenPreferences),
                MenuItem::separator(),
                MenuItem::action("Quit Lumina", QuitApp),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open Library…", OpenLibrary),
                MenuItem::action("Import…", ImportMedia),
                MenuItem::action("Rescan Library", RescanLibrary),
                MenuItem::separator(),
                MenuItem::action("New Smart Album…", NewSmartAlbum),
                MenuItem::separator(),
                MenuItem::action("Import XMP Metadata", ImportXmp),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Command Palette…", ShowCommandPalette),
                MenuItem::separator(),
                MenuItem::action("Toggle Sidebar", ToggleSidebar),
                MenuItem::action("Toggle Inspector", ToggleInspector),
                MenuItem::separator(),
                MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("Keyboard Shortcuts", ShowShortcuts),
                MenuItem::action("About Lumina", ShowAbout),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_has_core_commands() {
        let menus = build_menus();
        let names: Vec<&str> = menus.iter().map(|menu| menu.name.as_ref()).collect();
        assert_eq!(names, vec!["Lumina", "File", "View", "Help"]);

        let app_items = &menus[0].items;
        assert!(app_items
            .iter()
            .any(|item| matches!(item, MenuItem::Action { name, .. } if name == "Preferences…")));
        assert!(menus[1]
            .items
            .iter()
            .any(|item| matches!(item, MenuItem::Action { name, .. } if name == "Open Library…")));
    }
}
