//! The app's one-line text fields (search, palette, inspector
//! title/caption/date/place, tag names, smart-album values, video link).
//!
//! They all edit through one GPUI Kit `InputState` ([`FieldInput`]): caret,
//! selection, copy/cut/paste, undo, and IME. Only one field takes typing at a
//! time (`text_target`), so the input follows it: render loads the field's
//! `String` into it and focuses it in the window that draws the field, and
//! each edit is copied back, so the `String` stays the source of truth for
//! the keymap and the commits.
//!
//! Printable keys go to the input, not the keymap, so Shift, ⌥ characters,
//! dead keys and composition arrive as the text the user meant. The input
//! passes Enter and Escape on to the keymap. A one-line input has no ↑/↓
//! handler, so those reach the keymap (suggestions, palette rows) as well;
//! Tab and the rest the fields never edit with are sent there by
//! `field_input_actions`, and ⌘V keeps the one-line clean-up.

use gpui::base::input::{
    Backspace, DeleteToBeginningOfLine, IndentInline, Input, InputAccessibility, InputBaseState,
    InputEditorStyle, InputEvent, InputModeKind, InputState, OutdentInline, Paste, SelectAll,
    Textarea, TextareaState,
};
use gpui::*;
use lumina_core::text::{is_disallowed, MAX_NAME_CHARS, MAX_TEXT_CHARS};

use crate::app::smart_draft::DraftFocus;
use crate::app::templates::TemplateField;
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
    TagManagerAlias,
    TagManagerSearch,
    Search,
    NetworkImportUrl,
    /// A metadata template editor field.
    Template(TemplateField),
}

impl TextTarget {
    /// Fields the inspector draws, in the floating window while it is open.
    fn in_inspector(self) -> bool {
        use TextTarget::*;
        matches!(self, Tag | Title | Date | Location | Caption)
    }

    /// Captions keep their line breaks (`clean_caption`), so they edit in a
    /// multi-line input: Enter saves, ⇧Enter starts a new line.
    fn multi_line(self) -> bool {
        self == TextTarget::Caption
    }

    /// What the empty field shows, and what VoiceOver calls it.
    fn placeholder_and_label(self) -> (&'static str, &'static str) {
        use TextTarget::*;
        match self {
            Palette => ("Type a command, album, tag or year…", "Command"),
            AlbumDraft => ("", "Smart album field"),
            Tag => ("Type a tag, Enter to add", "Tag"),
            Title => ("Type a title... (Enter to save, Esc to cancel)", "Title"),
            Date => ("YYYY-MM-DD HH:MM:SS", "Date"),
            Location => (
                "latitude, longitude (Enter to save, Esc to cancel)",
                "Location",
            ),
            Caption => ("Type caption... (Enter to save, Esc to cancel)", "Caption"),
            TagRename | TagManagerRename => ("", "Tag name"),
            TagManagerAlias => ("Type an alias, Enter to add", "Alias"),
            TagManagerSearch => ("Filter tags by name…", "Filter tags"),
            Search => ("", "Search"),
            NetworkImportUrl => ("Paste a video link (⌘V)", "Video link"),
            Template(field) => template_placeholder_and_label(field),
        }
    }
}

/// Template editor fields: the label is the field's name.
fn template_placeholder_and_label(field: TemplateField) -> (&'static str, &'static str) {
    match field {
        TemplateField::Name => ("Name this template", "Template name"),
        TemplateField::Creator => ("Your name", "Creator"),
        TemplateField::Copyright => ("© 2026 Your Name", "Copyright"),
        TemplateField::UsageTerms => ("All rights reserved", "Usage terms"),
        TemplateField::ContactEmail => ("name@example.com", "Contact email"),
        TemplateField::ContactUrl => ("https://example.com", "Contact website"),
        TemplateField::Keywords => ("Keywords, separated by commas", "Keywords"),
    }
}

/// The input drawn for the active field.
#[derive(Clone)]
pub(crate) enum FieldEditor {
    Line(Entity<InputState>),
    Lines(Entity<TextareaState>),
}

impl FieldEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            FieldEditor::Line(state) => state.focus_handle(cx),
            FieldEditor::Lines(state) => state.focus_handle(cx),
        }
    }
}

impl IntoElement for FieldEditor {
    type Element = AnyElement;

    fn into_element(self) -> AnyElement {
        match self {
            FieldEditor::Line(state) => Input::new(&state).into_any_element(),
            FieldEditor::Lines(state) => Textarea::new(&state).into_any_element(),
        }
    }
}

/// The inputs behind whichever field takes typing: one line, and a
/// multi-line one made the first time a caption is edited.
pub(crate) struct FieldInput {
    line: Entity<InputState>,
    lines: Option<Entity<TextareaState>>,
    target: Option<TextTarget>,
    /// The smart-album control `AlbumDraft` edits (the title or a value).
    draft: DraftFocus,
    /// The text both sides last agreed on. The side that differs from it
    /// changed: the buffer (app code) wins at render, the input on `Change`.
    synced: String,
    _events: Vec<Subscription>,
}

impl FieldInput {
    fn editor(&self, multi_line: bool) -> Option<FieldEditor> {
        if multi_line {
            self.lines.clone().map(FieldEditor::Lines)
        } else {
            Some(FieldEditor::Line(self.line.clone()))
        }
    }
}

/// Loads `text` into an input for `target`. The text goes in before
/// any validator, so a stored value the validator would refuse (a legacy
/// over-long tag) still loads and can be fixed.
fn load_field<M: InputModeKind>(
    state: &mut InputBaseState<M>,
    target: TextTarget,
    text: String,
    window: &mut Window,
    cx: &mut Context<InputBaseState<M>>,
) {
    // Unset colors fall back to GPUI Kit's light palette (a black caret).
    let mut selection: Hsla = crate::theme::accent().into();
    selection.a = 0.4;
    state.set_editor_style(InputEditorStyle {
        caret: crate::theme::text().into(),
        selection,
        ..Default::default()
    });
    let (placeholder, label) = target.placeholder_and_label();
    state.set_placeholder(placeholder, window, cx);
    state.set_accessibility(
        InputAccessibility::new(Some(Role::TextInput)).with_label(Some(label.into())),
    );
    let end = text.len();
    state.set_value(text, window, cx);
    // A caption would start at its first line; every field opens at its end.
    state.set_selected_range(end..end, cx);
}

/// The span of `new` (in UTF-16 units) that differs from `old`, and the
/// text of `old` it replaced: everything between their common start and end.
fn edited_span(old: &str, new: &str) -> (std::ops::Range<usize>, String) {
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let (old_rest, new_rest) = (&old[prefix..], &new[prefix..]);
    let suffix: usize = old_rest
        .chars()
        .rev()
        .zip(new_rest.chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let utf16 = |text: &str| text.encode_utf16().count();
    let start = utf16(&new[..prefix]);
    let end = utf16(&new[..new.len() - suffix]);
    (start..end, old_rest[..old_rest.len() - suffix].to_string())
}

impl LuminaApp {
    /// The input to draw for `target` while that field takes typing.
    pub(crate) fn field_input_for(&self, target: TextTarget) -> Option<FieldEditor> {
        let field = self.field_input.as_ref()?;
        if field.target != Some(target) {
            return None;
        }
        field.editor(target.multi_line())
    }

    /// Runs at the top of each window's render: points the input at the
    /// active field, loads text the app changed (a cleared search, an
    /// applied suggestion), and moves focus between the input and `root`.
    /// The floating inspector window owns the inspector's fields; the main
    /// window owns the rest.
    pub(crate) fn sync_field_input(
        &mut self,
        window: &mut Window,
        root: &FocusHandle,
        floating_inspector: bool,
        cx: &mut Context<Self>,
    ) {
        let target = self.text_target();
        if self.field_input.is_none() {
            if target.is_none() {
                return;
            }
            // `main` sets up the input's key bindings and globals at launch;
            // the UI tests build the app without it.
            if !cx.has_global::<gpui::base::GlobalState>() {
                gpui::init(cx);
            }
            let line = cx.new(|cx| InputState::new(window, cx));
            let events = vec![
                cx.subscribe_in(&line, window, |this, state, event, window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value();
                        this.field_input_changed(value, window, cx);
                    }
                }),
            ];
            self.field_input = Some(FieldInput {
                line,
                lines: None,
                target: None,
                draft: self.album_draft.focus,
                synced: String::new(),
                _events: events,
            });
        }
        if target.is_some_and(TextTarget::multi_line)
            && self.field_input.as_ref().is_some_and(|f| f.lines.is_none())
        {
            // Enter goes on to the keymap, which saves; ⇧Enter adds a line.
            let lines = cx.new(|cx| TextareaState::new(window, cx).submit_on_enter(true));
            let events = cx.subscribe_in(&lines, window, |this, state, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = state.read(cx).value();
                    this.field_input_changed(value, window, cx);
                }
            });
            if let Some(field) = self.field_input.as_mut() {
                field.lines = Some(lines);
                field._events.push(events);
            }
        }
        let docked = self.floating_inspector_window.is_none();
        let owned = target.filter(|t| {
            if floating_inspector {
                t.in_inspector()
            } else {
                docked || !t.in_inspector()
            }
        });
        let text = owned.and_then(|t| self.text_buffer(t).cloned());
        let limit = owned.map(|t| self.text_limit(t));
        let draft = self.album_draft.focus;
        let Some(field) = self.field_input.as_mut() else {
            return;
        };
        let editor = owned.and_then(|t| field.editor(t.multi_line()));
        let (Some(target), Some(text), Some(limit), Some(editor)) = (owned, text, limit, editor)
        else {
            // Only the window that draws the field points the input at it.
            if target.is_none() {
                field.target = None;
            }
            for editor in [field.editor(false), field.editor(true)]
                .into_iter()
                .flatten()
            {
                if editor.focus_handle(cx).is_focused(window) {
                    window.focus(root, cx);
                }
            }
            return;
        };
        // Another smart-album control is the same target with another buffer.
        let reload = field.target != Some(target)
            || (target == TextTarget::AlbumDraft && field.draft != draft)
            || text != field.synced;
        field.target = Some(target);
        field.draft = draft;
        field.synced.clone_from(&text);
        let focus = editor.focus_handle(cx);
        let focusing = !focus.is_focused(window);
        match &editor {
            FieldEditor::Line(state) => state.update(cx, |state, cx| {
                if reload {
                    // The last field's validator would refuse the new text.
                    state.set_validator(|_, _| true, cx);
                    load_field(state, target, text, window, cx);
                    // Typing and paste stop at the field's limit and never
                    // insert what the catalog drops (see `clean_text`).
                    state.set_validator(
                        move |text, _| {
                            text.chars().count() <= limit && !text.chars().any(is_disallowed)
                        },
                        cx,
                    );
                }
                if focusing {
                    state.focus(window, cx);
                }
            }),
            FieldEditor::Lines(state) => state.update(cx, |state, cx| {
                if reload {
                    load_field(state, target, text, window, cx);
                }
                if focusing {
                    state.focus(window, cx);
                }
            }),
        }
        // A field opened from the other window (T in the main window with
        // the inspector floating) brings this one up.
        if focusing && !window.is_window_active() {
            window.activate_window();
        }
    }

    fn field_input_changed(
        &mut self,
        value: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.field_input.as_mut() else {
            return;
        };
        // A late event for text the app has since replaced is not an edit.
        let Some(target) = field.target.filter(|_| field.synced != value.as_ref()) else {
            return;
        };
        let value = value.to_string();
        let synced = field.synced.clone();
        if target.multi_line() {
            // A multi-line input takes no validator: refuse an edit the way
            // the one-line one does, by taking back what was just typed
            // (line breaks are fine, `clean_caption`).
            let limit = self.text_limit(target);
            let fits = |text: &str| {
                text.chars().count() <= limit
                    && !text.chars().any(|c| c != '\n' && is_disallowed(c))
            };
            if !fits(&value) && fits(&synced) {
                // Put back exactly what the edit replaced, however it was
                // made (one caret, several, a selection, a composition).
                let (range, old) = edited_span(&synced, &value);
                if let Some(FieldEditor::Lines(state)) = self.field_input_for(target) {
                    state.update(cx, |state, cx| {
                        state.replace_text_in_range(Some(range), &old, window, cx)
                    });
                }
                return;
            }
        }
        let Some(field) = self.field_input.as_mut() else {
            return;
        };
        let synced = std::mem::replace(&mut field.synced, value.clone());
        let Some(buffer) = self.text_buffer(target) else {
            return;
        };
        // The app changed the field since the last render (cleared it, say):
        // its text wins and the next render loads it.
        if *buffer != synced {
            if let Some(field) = self.field_input.as_mut() {
                field.synced = synced;
            }
            return;
        }
        *buffer = value;
        self.text_edited(target, cx);
    }

    /// Runs `key` through the keymap instead of the input when `wanted` says
    /// the active field has no use for it.
    fn field_key(
        &mut self,
        key: &'static str,
        wanted: fn(&mut Self, TextTarget) -> bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.text_target() else {
            return;
        };
        if !wanted(self, target) {
            return;
        }
        // `Keystroke::parse` needs the OS to read "secondary" (⌘ on macOS).
        let (secondary, key) = match key.strip_prefix("secondary-") {
            Some(key) => (true, key),
            None => (false, key),
        };
        let Ok(mut keystroke) = Keystroke::parse(key) else {
            return;
        };
        if secondary {
            keystroke.modifiers = Modifiers::secondary_key(cx);
        }
        cx.stop_propagation();
        let event = KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        };
        self.handle_key_down(&event, window, cx);
    }

    /// ⌘V into the active field: one line, with line breaks, tabs and other
    /// characters the catalog never keeps turned into spaces, and cut to the
    /// room left. The input's own paste would refuse the whole text instead.
    fn paste_into_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.text_target() else {
            return;
        };
        let Some(editor) = self.field_input_for(target) else {
            return;
        };
        cx.stop_propagation();
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        let line: String = text
            .trim_matches(['\n', '\r'])
            .chars()
            .map(|c| if is_disallowed(c) { ' ' } else { c })
            .collect();
        let limit = self.text_limit(target);
        fn paste<M: InputModeKind>(
            state: &mut InputBaseState<M>,
            line: &str,
            limit: usize,
            window: &mut Window,
            cx: &mut Context<InputBaseState<M>>,
        ) {
            let kept = state.value().chars().count() - state.selected_value().chars().count();
            let text: String = line.chars().take(limit.saturating_sub(kept)).collect();
            state.replace(text, window, cx);
        }
        match editor {
            FieldEditor::Line(state) => {
                state.update(cx, |state, cx| paste(state, &line, limit, window, cx))
            }
            FieldEditor::Lines(state) => {
                state.update(cx, |state, cx| paste(state, &line, limit, window, cx))
            }
        }
    }

    /// The active field, in the same precedence as `handle_modal_key_down`.
    pub(crate) fn text_target(&self) -> Option<TextTarget> {
        use TextTarget::*;
        if self.downloads.confirm.is_some() {
            return None;
        }
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
        // Every stage of the dialog takes the keys; only the first types.
        if let Some(ref stage) = self.network_import.stage {
            return matches!(
                stage,
                crate::app::network_import::NetworkImportStage::Input { .. }
            )
            .then_some(NetworkImportUrl);
        }
        if let Some(ref editor) = self.templates.editor {
            return Some(Template(editor.focus));
        }
        if self.downloads.open {
            return None;
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
            } else if self.tag_manager.alias_editing {
                TagManagerAlias
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
            TagManagerAlias => &mut self.tag_manager.alias_buffer,
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
            Template(field) => self.templates.editor.as_mut()?.value_mut(field),
        })
    }

    /// Longest text `target` takes: names (tags, smart album titles) are
    /// short, everything else gets the catalog's text cap.
    fn text_limit(&self, target: TextTarget) -> usize {
        use TextTarget::*;
        match target {
            Tag | TagRename | TagManagerRename | TagManagerAlias => MAX_NAME_CHARS,
            Template(TemplateField::Name) => MAX_NAME_CHARS,
            AlbumDraft if self.album_draft.focus == DraftFocus::Title => MAX_NAME_CHARS,
            _ => MAX_TEXT_CHARS,
        }
    }

    /// Side effects the keymap used to run after each typed character.
    fn text_edited(&mut self, target: TextTarget, cx: &mut Context<Self>) {
        match target {
            // New text means new suggestions; the old highlight is stale.
            TextTarget::Tag => self.keywords.suggestion_focus = None,
            TextTarget::Palette => {
                self.command_selected = 0;
                self.command_scroll.scroll_to_item(1);
            }
            TextTarget::AlbumDraft if self.album_draft.focus != DraftFocus::Title => {
                self.refresh_draft_preview();
            }
            TextTarget::Search => {
                self.reset_search_suggestions();
                self.scope_search_to_library();
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
}

/// Sends Tab to the keymap (smart-album focus), and Backspace in an empty field (it removes the last
/// tag) and ⌘V to the app. Each window root that draws fields calls this.
pub(crate) fn field_input_actions(root: Div, app: &Entity<LuminaApp>) -> Div {
    fn always(_: &mut LuminaApp, _: TextTarget) -> bool {
        true
    }
    fn empty(app: &mut LuminaApp, target: TextTarget) -> bool {
        app.text_buffer(target).is_some_and(|text| text.is_empty())
    }
    // ⌘A checks every shown tag, as it did before the filter took a caret.
    fn tag_filter(_: &mut LuminaApp, target: TextTarget) -> bool {
        target == TextTarget::TagManagerSearch
    }
    fn reroute<A: Action>(
        root: Div,
        app: &Entity<LuminaApp>,
        key: &'static str,
        wanted: fn(&mut LuminaApp, TextTarget) -> bool,
    ) -> Div {
        let app = app.clone();
        root.capture_action(move |_: &A, window, cx| {
            app.update(cx, |app, cx| app.field_key(key, wanted, window, cx));
        })
    }
    let root = reroute::<IndentInline>(root, app, "tab", always);
    let root = reroute::<OutdentInline>(root, app, "shift-tab", always);
    let root = reroute::<Backspace>(root, app, "backspace", empty);
    let root = reroute::<DeleteToBeginningOfLine>(root, app, "secondary-backspace", empty);
    let root = reroute::<SelectAll>(root, app, "secondary-a", tag_filter);
    let app = app.clone();
    root.capture_action(move |_: &Paste, window, cx| {
        app.update(cx, |app, cx| app.paste_into_field(window, cx));
    })
}

#[cfg(test)]
mod tests {
    use super::edited_span;

    #[core::prelude::v1::test]
    fn edited_span_finds_what_any_edit_replaced() {
        assert_eq!(
            edited_span("ab", "axb"),
            (1..2, String::new()),
            "one insert"
        );
        assert_eq!(
            edited_span("abcd", "aXbcYd"),
            (1..5, "bc".to_string()),
            "two carets: the whole span between them goes back"
        );
        assert_eq!(
            edited_span("abc", "aXYc"),
            (1..3, "b".to_string()),
            "a selection replaced"
        );
        assert_eq!(
            edited_span("aa", "aaa"),
            (2..3, String::new()),
            "a repeated character"
        );
        assert_eq!(
            edited_span("é😀", "é😀😀"),
            (3..5, String::new()),
            "UTF-16 offsets"
        );
    }
}
