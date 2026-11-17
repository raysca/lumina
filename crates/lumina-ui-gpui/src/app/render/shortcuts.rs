//! The keyboard shortcut sheet.

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// The shortcut sheet: titled sections of (keys, what they do). Keep in
/// step with `app/keymap`.
const SHORTCUT_SECTIONS: [(&str, &[(&str, &str)]); 12] = [
    (
        "GENERAL",
        &[
            ("⌘K", "Command palette"),
            ("⌘F", "Focus search"),
            ("⌘O", "Open library…"),
            ("⇧⌘I", "Import…"),
            ("⌘,", "Preferences"),
            ("↑ / ↓ in Settings", "Previous / next category"),
            (
                "Tab / ⇧Tab, Return / Space in Settings",
                "Move between / press buttons",
            ),
            (
                "← / → in General and Downloads",
                "Launch view / downloads at a time",
            ),
            ("⌘R", "Rescan library"),
            ("⌘Z / ⇧⌘Z", "Undo / redo"),
            ("⌘[ / ⌘]", "Back / Forward"),
            ("⌘N", "New album"),
            ("⌥⌘N", "New smart album"),
            ("⌥⌘L", "Show or hide downloads"),
            ("⌃⌘S", "Show or hide sidebar"),
            ("I", "Toggle inspector"),
            ("⌘I", "Get Info window"),
            ("F", "Favorite"),
            ("⇧⌘R", crate::i18n::platform::SHOW),
            ("⌥⌘C", "Copy file path"),
            ("⌘E", "Edit in external editor"),
            ("⌥⌘E", "Edit a copy in external editor"),
            ("Esc", "Close / go back"),
            ("⌘/", "Toggle this help"),
        ],
    ),
    (
        "BROWSING",
        &[
            ("← ↑ → ↓", "Move selection"),
            ("⇧ + arrows", "Extend selection"),
            ("⌘A", "Select all"),
            ("⇧⌘A", "Add to album…"),
            ("⌫ in an album", "Remove from album"),
            ("Home / End", "First / last item"),
            ("PageUp / PageDown", "Jump four rows"),
            ("⌘+ / ⌘-", "Grid density"),
            ("Space / Enter", "Open in viewer"),
            ("T", "Tag selection"),
            ("R / ⇧R", "Rotate right / left"),
            ("P / X / U", "Pick / reject / remove flag"),
            ("1–5 / 0", "Rate / clear rating"),
            ("⌫", "Delete…"),
            ("C", "Compare the selection (2–9)"),
        ],
    ),
    (
        "COMPARE",
        &[
            ("← / → / Tab", "Focus another pane"),
            ("P / X / U, 0–5", "Flag / rate the focused pane"),
            ("⌫", "Remove the pane (keeps the file)"),
            ("Enter", "Open in viewer (Esc returns)"),
            ("+ / - / Z / ⌘0–⌘4", "Zoom every pane"),
            ("⌘Z / ⇧⌘Z", "Undo / redo a flag or rating"),
            ("Esc", "Close compare"),
        ],
    ),
    (
        "REVIEW DUPLICATES",
        &[
            ("↑ / ↓", "Previous / next group"),
            ("← / → or 1–9", "Keep another copy"),
            ("S", "Skip group"),
            ("Enter", "Merge group"),
            ("⌘↓", "Open the selected copy or the keeper"),
            ("⌘K Skip All Groups", "Skip every group"),
        ],
    ),
    (
        "SIMILAR SHOTS (DUPLICATES)",
        &[
            ("↑ / ↓", "Previous / next stack"),
            ("Enter on a stack", "Keep picks, reject the rest"),
            ("⇧Enter", "Reject all but the first"),
            (
                "⌘K Show Similar Shots",
                "This tab (⌘K Show Exact Copies: the other)",
            ),
        ],
    ),
    (
        "MAP (PLACES)",
        &[
            (
                "⌘K Show Map / Show Grid",
                "Places on the map / grid (View > Show Map)",
            ),
            ("Tab, → or ↓", "Focus the next place east"),
            ("⇧Tab, ← or ↑", "Focus the previous place west"),
            ("Enter", "Show the focused place's photos"),
            ("⌘+ / ⌘-", "Zoom the map (keeps the focused place centred)"),
            ("Drag", "Pan the map"),
            ("Esc", "Back to the grid"),
        ],
    ),
    (
        "VIEWER",
        &[
            ("[ / ]", "Previous / next"),
            ("← / →", "Previous / next photo"),
            ("Enter / Esc", "Close viewer"),
            ("⌃⌘F", "Full screen"),
            ("H", "Hide / show controls"),
            ("+ / -", "Zoom in / out"),
            ("Pinch / ⌘ scroll", "Zoom at pointer"),
            ("Scroll / drag", "Pan when zoomed"),
            ("⌘0 / ⌘1 / ⌘2 / ⌘4", "Fit / 100% / 200% / 400%"),
            ("⌘3", "Fill"),
            ("Z", "Toggle 1:1 zoom"),
            ("O", "Open in default app"),
            ("P / X / U", "Pick / reject / remove flag, then next"),
            ("1–5 / 0", "Rate / clear rating, then next"),
            ("R / ⇧R", "Rotate right / left"),
            ("E", "Edit photo"),
        ],
    ),
    (
        "SLIDESHOW",
        &[
            ("⌥⌘S", "Play the view from the selection"),
            ("Space", "Pause / resume"),
            ("← / →", "Previous / next slide"),
            ("Esc", "End the slideshow"),
        ],
    ),
    (
        "EDITING A PHOTO",
        &[
            ("R / ⇧R", "Rotate right / left"),
            ("- / =", "Darker / brighter (⅓ stop)"),
            ("A", "Auto enhance on / off"),
            ("C", "Crop; again: next aspect"),
            ("⇧C", "Crop on / off (keeps the crop)"),
            ("← → ↑ ↓", "Move the crop 1 % (⇧ 10 %)"),
            ("⌥ arrows", "Resize the crop"),
            ("Enter", "Done (⌘Z undoes)"),
            ("Esc", "Cancel (while cropping: leave crop)"),
        ],
    ),
    (
        "VIDEO",
        &[
            ("Space / K", "Play / pause"),
            ("J / ←", "Back 5 s"),
            ("→", "Forward 5 s"),
            (", / .", "Previous / next frame"),
            ("L", "Loop"),
            ("⌥I / ⌥O", "Set loop start / end"),
            ("⌥X", "Clear loop range"),
            ("⌥← / ⌥→", "Previous / next chapter"),
            ("M", "Mute"),
            ("↑ / ↓", "Volume up / down"),
            ("⌥P", "Picture-in-Picture on / off"),
        ],
    ),
    (
        "TAGS PAGE",
        &[
            ("Type", "Filter the keywords"),
            ("↑ / ↓", "Previous / next keyword"),
            ("Space", "Check the keyword"),
            ("⌘A", "Check / clear every shown keyword"),
            ("⌘M / ⌘⌫", "Merge / delete the checked keywords"),
            ("Enter", "Rename the keyword"),
            ("⇧⌘A", "Add an alias for the keyword"),
            ("⇧⌘M", "Move the keyword to another parent"),
            ("⇧F10", "Actions for the keyword"),
            ("Esc", "Clear the checks"),
        ],
    ),
    (
        "IN METADATA TEMPLATES",
        &[
            ("Tab / ⇧Tab", "Next / previous field"),
            ("↑ / ↓", "Previous / next template"),
            ("Enter", "Save the template"),
            ("⇧⌘⌫", "Delete the template (⌘Z undoes)"),
            ("Esc", "Close without saving"),
        ],
    ),
];

fn shortcut_display_text(text: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        text.replace("⌃⌘", "Ctrl+Super+")
            .replace("⇧⌘", "Ctrl+Shift+")
            .replace("⌥⌘", "Ctrl+Alt+")
            .replace("⌘", "Ctrl+")
            .replace("⌃", "Ctrl+")
            .replace("⌥", "Alt+")
            .replace("⇧", "Shift+")
    }
    #[cfg(not(target_os = "linux"))]
    {
        text.to_string()
    }
}

impl LuminaApp {
    /// Keyboard shortcut cheat sheet (Cmd+/), also serving as keyboard discoverability.
    pub(super) fn render_shortcuts_overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.shortcuts_overlay_open {
            return Empty.into_any_element();
        }
        div()
            .id("shortcuts-overlay")
            .debug_selector(|| "shortcuts-overlay".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::modal_dim())
            .flex()
            .justify_center()
            .items_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_shortcuts_overlay(cx);
                }),
            )
            .child(
                div()
                    .id("shortcuts-card")
                    .debug_selector(|| "shortcuts-card".to_string())
                    .w(px(520.0))
                    .max_h(px(560.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.shortcuts_scroll)
                    .bg(theme::bg_1())
                    .border_1()
                    .border_color(theme::line())
                    .rounded(theme::RADIUS_LG)
                    .shadow_2xl()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme::t1())
                                    .child("Keyboard Shortcuts"),
                            )
                            .child(
                                div()
                                    .id("shortcuts-close")
                                    .debug_selector(|| "shortcuts-close".to_string())
                                    .cursor_pointer()
                                    .child(render_icon(Icon::X, px(12.0), theme::t1()))
                                    .tooltip(theme::tooltip_label("Close"))
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.close_shortcuts_overlay(cx);
                                    })),
                            ),
                    )
                    .children(SHORTCUT_SECTIONS.into_iter().map(|(title, rows)| {
                        div()
                            .pt_2()
                            .flex()
                            .flex_col()
                            .child(theme::section_label(title))
                            .children(rows.iter().map(|&(keys, description)| {
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .items_center()
                                    .py_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme::t1())
                                            .child(shortcut_display_text(description)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family(theme::FONT_MONO)
                                            .text_color(theme::t2())
                                            .child(shortcut_display_text(keys)),
                                    )
                            }))
                    })),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::SHORTCUT_SECTIONS;

    fn says(keys: &str) -> Option<&'static str> {
        SHORTCUT_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter())
            .find(|(k, _)| *k == keys)
            .map(|r| r.1)
    }

    /// Regression: the sheet listed "Z / O" for 1:1 zoom, but O opens the
    /// file in its default app (`app/keymap/viewer.rs`).
    #[core::prelude::v1::test]
    fn the_sheet_says_what_z_and_o_do() {
        assert_eq!(says("Z"), Some("Toggle 1:1 zoom"), "Z toggles 1:1 zoom");
        assert_eq!(
            says("O"),
            Some("Open in default app"),
            "O opens the default app, it does not zoom"
        );
    }

    /// Zoom presets moved to ⌘digits; plain digits are reserved for ratings.
    #[core::prelude::v1::test]
    fn the_sheet_lists_zoom_presets_on_command_digits() {
        assert_eq!(says("⌘3"), Some("Fill"), "⌘3 is Fill");
        assert_eq!(
            says("⌘0 / ⌘1 / ⌘2 / ⌘4"),
            Some("Fit / 100% / 200% / 400%"),
            "the presets are on ⌘digits"
        );
        assert_eq!(says("0 / 1 / 2 / 4"), None, "plain digits no longer zoom");
    }

    /// The sheet described Quick Look (there is none), and ← / → as
    /// previous/next even on videos, where they seek.
    #[core::prelude::v1::test]
    fn the_sheet_describes_space_and_arrows_as_they_work() {
        assert_eq!(says("Space / Enter"), Some("Open in viewer"));
        assert_eq!(
            says("← / →"),
            Some("Previous / next photo"),
            "on a video ← / → seek instead"
        );
        for (_, rows) in SHORTCUT_SECTIONS {
            for (keys, what) in rows {
                assert!(
                    !what.contains("Quick look"),
                    "{keys}: there is no Quick Look"
                );
            }
        }
    }
}
