//! Platform text input for the app's one-line fields (search, palette,
//! inspector title/caption/date/place, tag names, smart-album values).
//!
//! Printable keys are left to GPUI's input handler instead of the keymap, so
//! Shift, ⌥ characters, dead keys and IME composition arrive as the text the
//! user meant. The keymap keeps Enter, Escape, Backspace and the arrows.
//! ponytail: edits always happen at the end of the field (no caret or
//! selection); add a caret model when a field needs mid-text editing.

use std::ops::Range;

use gpui::*;
use lumina_core::text::{is_disallowed, MAX_NAME_CHARS, MAX_TEXT_CHARS};

use crate::app::smart_draft::DraftFocus;
use crate::app::LuminaApp;

/// The field typed text goes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TextTarget {
    Palette,
    AlbumDraft,
    Tag,
    Title,
    Date,
    Location,
    Caption,
    TagRename,
    TagManagerRename,
    TagManagerSearch,
    Search,
    NetworkImportUrl,
}

/// An IME composition in progress: the field it belongs to, its UTF-16
/// range there, and the marked text itself.
#[derive(Clone, Debug)]
pub(crate) struct MarkedText {
    target: TextTarget,
    range: Range<usize>,
    text: String,
}

impl LuminaApp {
    /// Forgets a composition whose field closed, changed, or no longer holds
    /// the marked text (another field, or the same field reopened for
    /// another item). Runs every render and before each edit, so a stale
    /// range can never replace text.
    pub(crate) fn drop_stale_text_mark(&mut self) {
        let Some(mark) = self.text_marked.take() else {
            return;
        };
        if self.text_target() != Some(mark.target) {
            return;
        }
        let Some(buffer) = self.text_buffer(mark.target) else {
            return;
        };
        let start = byte_offset(buffer, mark.range.start);
        let end = byte_offset(buffer, mark.range.end).max(start);
        if buffer[start..end] == mark.text {
            self.text_marked = Some(mark);
        }
    }

    /// The active field, in the same precedence as `handle_modal_key_down`.
    pub(crate) fn text_target(&self) -> Option<TextTarget> {
        use TextTarget::*;
        if self.command_palette_open {
            return Some(Palette);
        }
        if self.shortcuts_overlay_open
            || self.preferences_open
            || self.delete_confirm_state.is_some()
            || self.album_draft.confirm_delete.is_some()
            || self.import.dialog.is_some()
        {
            return None;
        }
        if let Some(crate::app::network_import::NetworkImportStage::Input { .. }) =
            self.network_import.stage
        {
            return Some(NetworkImportUrl);
        }
        if self.album_draft.modal_open {
            return (self.album_draft.popup.is_none() && self.draft_focus_is_typed())
                .then_some(AlbumDraft);
        }
        let target = if self.tag_input_active {
            Tag
        } else if self.caption.title_input_active {
            Title
        } else if self.caption.date_input_active {
            Date
        } else if self.caption.location_input_active {
            Location
        } else if self.caption.input_active {
            Caption
        } else if self.editing_tag.is_some() {
            TagRename
        } else if self.tag_manager.open {
            if self.tag_manager.editing.is_some() {
                TagManagerRename
            } else {
                TagManagerSearch
            }
        } else if self.search_focused {
            Search
        } else {
            return None;
        };
        Some(target)
    }

    fn text_buffer(&mut self, target: TextTarget) -> Option<&mut String> {
        use TextTarget::*;
        Some(match target {
            Palette => &mut self.command_query,
            AlbumDraft => match self.album_draft.focus {
                DraftFocus::Title => &mut self.album_draft.album_title,
                DraftFocus::Value(i) => &mut self.album_draft.rows.get_mut(i)?.text,
                _ => return None,
            },
            Tag => &mut self.tag_input_buffer,
            Title => &mut self.caption.title_input_buffer,
            Date => &mut self.caption.date_input_buffer,
            Location => &mut self.caption.location_input_buffer,
            Caption => &mut self.caption.input_buffer,
            TagRename => &mut self.editing_tag_buffer,
            TagManagerRename => &mut self.tag_manager.editing_buffer,
            TagManagerSearch => &mut self.tag_manager.search,
            Search => &mut self.search_query,
            NetworkImportUrl => {
                if let Some(crate::app::network_import::NetworkImportStage::Input {
                    ref mut url,
                    ..
                }) = self.network_import.stage
                {
                    url
                } else {
                    return None;
                }
            }
        })
    }

    /// Longest text `target` takes: names (tags, smart album titles) are
    /// short, everything else gets the catalog's text cap.
    fn text_limit(&self, target: TextTarget) -> usize {
        use TextTarget::*;
        match target {
            Tag | TagRename | TagManagerRename => MAX_NAME_CHARS,
            AlbumDraft if self.album_draft.focus == DraftFocus::Title => MAX_NAME_CHARS,
            _ => MAX_TEXT_CHARS,
        }
    }

    /// Side effects the keymap used to run after each typed character.
    fn text_edited(&mut self, target: TextTarget, cx: &mut Context<Self>) {
        match target {
            TextTarget::Palette => {
                self.command_selected = 0;
                self.command_scroll.scroll_to_item(1);
            }
            TextTarget::AlbumDraft if self.album_draft.focus != DraftFocus::Title => {
                self.refresh_draft_preview();
            }
            TextTarget::Search => {
                self.reset_search_suggestions();
                self.clear_selection(cx);
            }
            TextTarget::NetworkImportUrl => {
                if let Some(crate::app::network_import::NetworkImportStage::Input {
                    ref mut error,
                    ..
                }) = self.network_import.stage
                {
                    *error = None;
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// True when the keystroke is text for the active field; the keymap then
    /// leaves it to the platform input handler.
    pub(crate) fn leaves_key_for_text_input(&self, keystroke: &Keystroke) -> bool {
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.function {
            return false;
        }
        let Some(target) = self.text_target() else {
            return false;
        };
        // Space in the empty tag-manager filter checks the focused tag.
        if target == TextTarget::TagManagerSearch
            && keystroke.key == "space"
            && self.tag_manager.search.is_empty()
        {
            return false;
        }
        keystroke
            .key_char
            .as_deref()
            .is_some_and(|text| !text.is_empty() && !text.chars().any(char::is_control))
    }

    /// ⌘V into the active field. Fields are one line, so line breaks, tabs
    /// and other characters the catalog never keeps become spaces. Returns
    /// false when no field is active.
    pub(crate) fn paste_into_text_field(&mut self, cx: &mut Context<Self>) -> bool {
        if self.text_target().is_none() {
            return false;
        }
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        let line: String = text
            .trim_matches(['\n', '\r'])
            .chars()
            .map(|c| if is_disallowed(c) { ' ' } else { c })
            .collect();
        self.replace_text(None, &line, false, cx);
        true
    }

    fn replace_text(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        mark: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.text_target() else {
            return;
        };
        self.drop_stale_text_mark();
        let marked = self.text_marked.take().map(|mark| mark.range);
        // Enter and Tab also arrive here as "\n" / "\t" after the keymap ran;
        // the same characters `clean_text` drops never enter a field.
        let text: String = text.chars().filter(|&c| !is_disallowed(c)).collect();
        let limit = self.text_limit(target);
        let Some(buffer) = self.text_buffer(target) else {
            return;
        };
        let end = utf16_len(buffer);
        let range = range_utf16.or(marked).unwrap_or(end..end);
        let start = byte_offset(buffer, range.start);
        let stop = byte_offset(buffer, range.end).max(start);
        // Typing stops at the field's limit, so it shows what will be saved.
        let room =
            limit.saturating_sub(buffer.chars().count() - buffer[start..stop].chars().count());
        let text: String = text.chars().take(room).collect();
        if start == stop && text.is_empty() {
            return;
        }
        buffer.replace_range(start..stop, &text);
        let start_utf16 = utf16_len(&buffer[..start]);
        if mark && !text.is_empty() {
            self.text_marked = Some(MarkedText {
                target,
                range: start_utf16..start_utf16 + utf16_len(&text),
                text,
            });
        }
        self.text_edited(target, cx);
    }
}

/// Registers `app` as the input handler of the window being painted while
/// one of its fields is active (`app.text_target()`; callers pass it in
/// because the main window renders inside an update of `app`). `focus` is
/// that window's root focus handle.
pub(crate) fn text_input_handler(
    active: bool,
    app: &Entity<LuminaApp>,
    focus: &FocusHandle,
) -> Option<AnyElement> {
    if !active {
        return None;
    }
    let (app, focus) = (app.clone(), focus.clone());
    Some(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, app), cx);
            },
        )
        .absolute()
        .size_full()
        .into_any_element(),
    )
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Byte offset of a UTF-16 offset, clamped to the text and to char bounds.
fn byte_offset(text: &str, utf16: usize) -> usize {
    let mut seen = 0;
    for (byte, c) in text.char_indices() {
        if seen >= utf16 {
            return byte;
        }
        seen += c.len_utf16();
    }
    text.len()
}

impl EntityInputHandler for LuminaApp {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let target = self.text_target()?;
        let buffer = self.text_buffer(target)?;
        let start = byte_offset(buffer, range.start);
        let end = byte_offset(buffer, range.end).max(start);
        *adjusted_range = Some(utf16_len(&buffer[..start])..utf16_len(&buffer[..end]));
        Some(buffer[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let target = self.text_target()?;
        let end = utf16_len(self.text_buffer(target)?);
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        // Stale marks are dropped each render (`drop_stale_text_mark`).
        let mark = self.text_marked.as_ref()?;
        (self.text_target() == Some(mark.target)).then(|| mark.range.clone())
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.text_marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text(range, text, false, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text(range, new_text, true, cx);
    }

    // ponytail: the IME candidate window anchors to the window, not the
    // field; pass field bounds in when that placement matters.
    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod unit {
    use super::byte_offset;

    #[core::prelude::v1::test]
    fn utf16_offsets_map_to_char_boundaries() {
        let text = "a😀é";
        assert_eq!(byte_offset(text, 0), 0);
        assert_eq!(byte_offset(text, 1), 1);
        assert_eq!(byte_offset(text, 3), 5, "the emoji is two UTF-16 units");
        assert_eq!(byte_offset(text, 4), text.len());
        assert_eq!(byte_offset(text, 99), text.len(), "past the end clamps");
    }
}
