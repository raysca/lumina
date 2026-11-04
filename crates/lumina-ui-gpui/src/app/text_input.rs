//! Platform text input for the app's one-line fields (search, palette,
//! inspector title/caption/date/place, tag names, smart-album values).
//!
//! Printable keys are left to GPUI's input handler instead of the keymap, so
//! Shift, ⌥ characters, dead keys and IME composition arrive as the text the
//! user meant. The keymap keeps Enter, Escape, Backspace and the arrows.
//!
//! Fields that `uses_input` edit through one GPUI Kit `InputState`
//! ([`FieldInput`]) with a caret, selection and copy/cut; Enter and Escape
//! still reach the keymap, and ↑/↓ and ⌘V are routed to the app. The other
//! fields still edit at the end through the app's own `EntityInputHandler`
//! below until they move to the input too.

use std::ops::Range;

use gpui::base::input::{InputEvent, InputState};
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

impl TextTarget {
    /// Fields edited through [`FieldInput`] (caret, selection, copy/cut,
    /// IME). The rest still type at the end through the app's own handler.
    pub(crate) fn uses_input(self) -> bool {
        matches!(self, TextTarget::Search)
    }
}

/// The one GPUI Kit `InputState` behind every field that `uses_input`. Only
/// one field takes typing at a time, so the state follows `text_target()`:
/// render loads the field's buffer into it and focuses it, and each edit is
/// copied back into the buffer, which stays the app's source of truth.
pub(crate) struct FieldInput {
    pub(crate) state: Entity<InputState>,
    target: Option<TextTarget>,
    /// The text both sides last agreed on. The side that differs from it
    /// changed: the buffer (app code) wins at render, the input on `Change`.
    synced: String,
    _events: Subscription,
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

    /// The input state of `target` while that field takes typing.
    pub(crate) fn field_input_for(&self, target: TextTarget) -> Option<&Entity<InputState>> {
        let field = self.field_input.as_ref()?;
        (field.target == Some(target)).then_some(&field.state)
    }

    /// Runs at the top of render: points the shared input at the active
    /// field, loads text the app changed (a cleared search, an applied
    /// suggestion), and moves keyboard focus between it and the root.
    pub(crate) fn sync_field_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.text_target().filter(|t| t.uses_input());
        if self.field_input.is_none() {
            if target.is_none() {
                return;
            }
            // Input key bindings and the base layer's globals. `main` never
            // needs a field before this, and tests build the app without it.
            if !cx.has_global::<gpui::base::GlobalState>() {
                gpui::init(cx);
            }
            let state = cx.new(|cx| InputState::new(window, cx));
            let _events = cx.subscribe_in(&state, window, |this, state, event, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.field_input_changed(state, cx);
                }
            });
            self.field_input = Some(FieldInput {
                state,
                target: None,
                synced: String::new(),
                _events,
            });
        }
        let text = target.and_then(|t| self.text_buffer(t).cloned());
        let limit = target.map(|t| self.text_limit(t));
        let Some(field) = self.field_input.as_mut() else {
            return;
        };
        let retarget = field.target != target;
        field.target = target;
        let reload = retarget || text.as_ref().is_some_and(|text| *text != field.synced);
        if let Some(text) = &text {
            field.synced.clone_from(text);
        }
        let state = field.state.clone();
        let focus = state.focus_handle(cx);
        let (Some(text), Some(limit)) = (text, limit) else {
            if focus.is_focused(window) {
                window.focus(&self.focus_handle, cx);
            }
            return;
        };
        state.update(cx, |state, cx| {
            if retarget {
                // Typing and paste stop at the field's limit and never
                // insert what the catalog drops (see `clean_text`).
                state.set_validator(
                    move |text, _| {
                        text.chars().count() <= limit && !text.chars().any(is_disallowed)
                    },
                    cx,
                );
            }
            if reload {
                state.set_value(text, window, cx);
            }
            if !focus.is_focused(window) {
                state.focus(window, cx);
            }
        });
    }

    fn field_input_changed(&mut self, state: &Entity<InputState>, cx: &mut Context<Self>) {
        let value = state.read(cx).value();
        let Some(field) = self.field_input.as_mut() else {
            return;
        };
        // A late event for text the app has since replaced is not an edit.
        let Some(target) = field.target.filter(|_| field.synced != value.as_ref()) else {
            return;
        };
        field.synced = value.to_string();
        let Some(buffer) = self.text_buffer(target) else {
            return;
        };
        buffer.clear();
        buffer.push_str(&value);
        self.text_edited(target, cx);
    }

    /// The active field's input, when it has one.
    fn active_field_input(&self) -> Option<(TextTarget, Entity<InputState>)> {
        let target = self.text_target()?;
        Some((target, self.field_input_for(target)?.clone()))
    }

    /// ↑/↓ belong to the app in one-line fields (suggestions, palette rows,
    /// tag lists), so they skip the input and go through the keymap.
    pub(crate) fn field_arrow(
        &mut self,
        key: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_field_input().is_none() {
            return;
        }
        cx.stop_propagation();
        let Ok(keystroke) = Keystroke::parse(key) else {
            return;
        };
        let event = KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        };
        self.handle_key_down(&event, window, cx);
    }

    /// ⌘V into an input-backed field, cleaned like `paste_into_text_field`
    /// and cut to the room left, instead of the input's own paste, which
    /// would refuse the whole text past the limit or with a tab in it.
    pub(crate) fn paste_into_field_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((target, state)) = self.active_field_input() else {
            return;
        };
        cx.stop_propagation();
        let line = clipboard_line(cx);
        let limit = self.text_limit(target);
        state.update(cx, |state, cx| {
            let kept = state.value().chars().count() - state.selected_value().chars().count();
            let text: String = line.chars().take(limit.saturating_sub(kept)).collect();
            state.replace(text, window, cx);
        });
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

    pub(crate) fn text_buffer(&mut self, target: TextTarget) -> Option<&mut String> {
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
        let line = clipboard_line(cx);
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

/// The clipboard as one line: line breaks, tabs and other characters the
/// catalog never keeps become spaces.
fn clipboard_line(cx: &App) -> String {
    let text = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .unwrap_or_default();
    text.trim_matches(['\n', '\r'])
        .chars()
        .map(|c| if is_disallowed(c) { ' ' } else { c })
        .collect()
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
