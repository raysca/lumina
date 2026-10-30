//! Localization scaffolding.
//!
//! Only English (en-US) ships today, but user-facing command labels go through
//! this catalog so additional locales can be added without touching call sites.
//! Select a locale with LUMINA_LANG (currently only "en-US" is understood).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    EnUs,
}

pub fn current_locale() -> Locale {
    match std::env::var("LUMINA_LANG").unwrap_or_default().as_str() {
        "en" | "en-US" | "en_US" => Locale::EnUs,
        _ => Locale::EnUs,
    }
}

/// Translates a catalog key, falling back to the key itself.
pub fn t(key: &str) -> String {
    match current_locale() {
        Locale::EnUs => en_us(key).unwrap_or(key).to_string(),
    }
}

fn en_us(key: &str) -> Option<&'static str> {
    Some(match key {
        "command.open_library" => "Open Library…",
        "command.rescan" => "Rescan Library",
        "command.preferences" => "Preferences…",
        "command.about" => "About Lumina",
        "command.shortcuts" => "Keyboard Shortcuts",
        "command.toggle_sidebar" => "Toggle Sidebar",
        "command.toggle_inspector" => "Toggle Inspector",
        "command.export" => "Export Selection…",
        "command.import_xmp" => "Import XMP Metadata",
        "command.sort_date" => "Sort by Date Taken",
        "command.sort_name" => "Sort by Name",
        "command.sort_size" => "Sort by File Size",
        "command.sort_rating" => "Sort by Rating",
        "command.sort_asc" => "Sort Ascending",
        "command.sort_desc" => "Sort Descending",
        "command.toggle_fit" => "Toggle Fit / Square Cards",
        "command.zoom_in" => "Zoom Grid In",
        "command.zoom_out" => "Zoom Grid Out",
        "command.rate_1" => "Rate 1 Star",
        "command.rate_2" => "Rate 2 Stars",
        "command.rate_3" => "Rate 3 Stars",
        "command.rate_4" => "Rate 4 Stars",
        "command.rate_5" => "Rate 5 Stars",
        "command.clear_rating" => "Clear Rating",
        "command.toggle_favorite" => "Toggle Favorite",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_and_unknown_keys() {
        assert_eq!(t("command.preferences"), "Preferences…");
        assert_eq!(t("missing.key"), "missing.key");
        assert_eq!(current_locale(), Locale::EnUs);
    }
}
