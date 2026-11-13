//! Metadata templates: a named set of rights fields and keywords kept in the
//! user config (they belong to the person, across libraries), applied to the
//! selection in one catalog transaction with one undo entry.

use gpui::*;
use lumina_core::catalog::keywords::normalize_path;
use lumina_core::Rights;
use serde::{Deserialize, Serialize};

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::i18n::t;

/// A saved template. Applying it overwrites only the rights fields it sets
/// and adds its keywords.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataTemplate {
    pub name: String,
    pub rights: Rights,
    pub keywords: Vec<String>,
}

/// The editor's fields, in Tab order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateField {
    Name,
    Creator,
    Copyright,
    UsageTerms,
    ContactEmail,
    ContactUrl,
    Keywords,
}

impl TemplateField {
    pub const ALL: [TemplateField; 7] = [
        TemplateField::Name,
        TemplateField::Creator,
        TemplateField::Copyright,
        TemplateField::UsageTerms,
        TemplateField::ContactEmail,
        TemplateField::ContactUrl,
        TemplateField::Keywords,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }

    /// The `template-field-{slug}` selector part.
    pub fn slug(self) -> &'static str {
        match self {
            TemplateField::Name => "name",
            TemplateField::Creator => "creator",
            TemplateField::Copyright => "copyright",
            TemplateField::UsageTerms => "usage-terms",
            TemplateField::ContactEmail => "contact-email",
            TemplateField::ContactUrl => "contact-url",
            TemplateField::Keywords => "keywords",
        }
    }

    /// Field label, also its accessible name.
    pub fn label(self) -> String {
        t(match self {
            TemplateField::Name => "template.field_name",
            TemplateField::Creator => "template.field_creator",
            TemplateField::Copyright => "template.field_copyright",
            TemplateField::UsageTerms => "template.field_usage_terms",
            TemplateField::ContactEmail => "template.field_contact_email",
            TemplateField::ContactUrl => "template.field_contact_url",
            TemplateField::Keywords => "template.field_keywords",
        })
    }
}

/// The open template editor: one template's fields as typed.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateEditor {
    /// Index into `config.templates`; `None` is a new template.
    pub editing: Option<usize>,
    pub values: [String; 7],
    pub focus: TemplateField,
    pub error: Option<String>,
}

impl TemplateEditor {
    fn for_template(editing: Option<usize>, template: &MetadataTemplate) -> Self {
        let field = |value: &Option<String>| value.clone().unwrap_or_default();
        let rights = &template.rights;
        TemplateEditor {
            editing,
            values: [
                template.name.clone(),
                field(&rights.creator),
                field(&rights.copyright),
                field(&rights.usage_terms),
                field(&rights.contact_email),
                field(&rights.contact_url),
                template.keywords.join(", "),
            ],
            focus: TemplateField::Name,
            error: None,
        }
    }

    pub fn value(&self, field: TemplateField) -> &str {
        &self.values[field.index()]
    }

    pub fn value_mut(&mut self, field: TemplateField) -> &mut String {
        &mut self.values[field.index()]
    }

    /// The template the fields describe, or why they do not make one.
    fn template(&self) -> Result<MetadataTemplate, String> {
        let name = self.value(TemplateField::Name).trim().to_string();
        if name.is_empty() {
            return Err(t("template.error_no_name"));
        }
        let keywords = self
            .value(TemplateField::Keywords)
            .split(',')
            .filter(|keyword| !keyword.trim().is_empty())
            .map(|keyword| normalize_path(keyword).map_err(|err| err.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let field = |field: TemplateField| Some(self.value(field).to_string());
        Ok(MetadataTemplate {
            name,
            rights: Rights {
                creator: field(TemplateField::Creator),
                copyright: field(TemplateField::Copyright),
                usage_terms: field(TemplateField::UsageTerms),
                contact_email: field(TemplateField::ContactEmail),
                contact_url: field(TemplateField::ContactUrl),
            }
            .cleaned(),
            keywords,
        })
    }
}

#[derive(Debug, Default)]
pub struct TemplateState {
    pub editor: Option<TemplateEditor>,
    /// The template the batch inspector's picker shows.
    pub picked: usize,
    /// The open palette lists template names (Apply Metadata Template…).
    pub palette_pick: bool,
}

impl LuminaApp {
    /// Opens the template editor on the first template, or a new one.
    /// Refused while another sheet, dialog or menu is up: the editor takes
    /// keys first but would draw under some of them.
    pub fn open_template_editor(&mut self, cx: &mut Context<Self>) {
        if self.import_blocked_by_modal() {
            self.show_warning_toast(t("template.error_other_dialog"), cx);
            return;
        }
        let editing = (!self.config.templates.is_empty()).then_some(0);
        self.load_template_in_editor(editing, cx);
    }

    /// Closes the editor; unsaved typing is discarded.
    pub fn close_template_editor(&mut self, cx: &mut Context<Self>) {
        self.templates.editor = None;
        cx.notify();
    }

    /// Shows template `editing` (`None`: a new one) in the editor,
    /// discarding unsaved typing.
    pub fn load_template_in_editor(&mut self, editing: Option<usize>, cx: &mut Context<Self>) {
        let editing = editing.filter(|&i| i < self.config.templates.len());
        let template = editing
            .map(|i| self.config.templates[i].clone())
            .unwrap_or_default();
        self.templates.editor = Some(TemplateEditor::for_template(editing, &template));
        cx.notify();
    }

    /// True when the editor's fields differ from the template it shows.
    pub fn template_editor_modified(&self) -> bool {
        self.templates.editor.as_ref().is_some_and(|editor| {
            let template = editor
                .editing
                .and_then(|i| self.config.templates.get(i))
                .cloned()
                .unwrap_or_default();
            editor.values != TemplateEditor::for_template(editor.editing, &template).values
        })
    }

    /// A list row click: shows that template, unless there is unsaved
    /// typing, which the editor keeps and asks to save or discard.
    pub fn pick_template_in_editor(&mut self, editing: Option<usize>, cx: &mut Context<Self>) {
        if self.refuse_template_discard(cx) {
            return;
        }
        self.load_template_in_editor(editing, cx);
    }

    /// A backdrop click closes the editor only when nothing is unsaved.
    pub fn dismiss_template_editor(&mut self, cx: &mut Context<Self>) {
        if !self.refuse_template_discard(cx) {
            self.close_template_editor(cx);
        }
    }

    /// Shows "save or discard" and returns true when the editor has unsaved
    /// typing.
    fn refuse_template_discard(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.template_editor_modified() {
            return false;
        }
        if let Some(editor) = self.templates.editor.as_mut() {
            editor.error = Some(t("template.error_unsaved"));
        }
        cx.notify();
        true
    }

    /// ↑/↓ in the editor: the previous or next template; past the last is
    /// a new one. With unsaved typing they stay in the field.
    pub fn step_template_in_editor(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.refuse_template_discard(cx) {
            return;
        }
        let Some(editor) = self.templates.editor.as_ref() else {
            return;
        };
        let count = self.config.templates.len();
        let current = editor.editing.unwrap_or(count) as isize;
        let next = (current + delta).clamp(0, count as isize) as usize;
        self.load_template_in_editor((next < count).then_some(next), cx);
    }

    /// Tab / ⇧Tab move between the editor's fields, wrapping.
    pub fn cycle_template_field(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(editor) = self.templates.editor.as_mut() {
            let len = TemplateField::ALL.len() as isize;
            let next = (editor.focus.index() as isize + delta).rem_euclid(len);
            editor.focus = TemplateField::ALL[next as usize];
            cx.notify();
        }
    }

    pub fn focus_template_field(&mut self, field: TemplateField, cx: &mut Context<Self>) {
        if let Some(editor) = self.templates.editor.as_mut() {
            editor.focus = field;
            cx.notify();
        }
    }

    /// Saves the editor's template to the config and closes the editor. A
    /// blank or duplicate name or a bad keyword keeps it open with the
    /// reason; a config write failure restores the old list (the banner
    /// says why) and keeps the typing.
    pub fn save_template_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.templates.editor.as_mut() else {
            return;
        };
        let template = match editor.template() {
            Ok(template) => template,
            Err(reason) => {
                editor.error = Some(reason);
                cx.notify();
                return;
            }
        };
        let editing = editor.editing;
        let taken = self.config.templates.iter().enumerate().any(|(i, other)| {
            Some(i) != editing && other.name.eq_ignore_ascii_case(&template.name)
        });
        if taken {
            editor.error = Some(t("template.error_name_taken"));
            cx.notify();
            return;
        }
        let previous = self.config.templates.clone();
        match editing {
            Some(i) => self.config.templates[i] = template.clone(),
            None => self.config.templates.push(template.clone()),
        }
        if !self.save_config() {
            self.config.templates = previous;
            cx.notify();
            return;
        }
        self.templates.editor = None;
        self.show_toast(t("template.saved").replace("{name}", &template.name), cx);
        cx.notify();
    }

    /// Deletes the template the editor shows (not a new one) from the
    /// config and closes the editor; the toast and ⌘Z undo it.
    pub fn delete_template_in_editor(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.templates.editor.as_ref().and_then(|e| e.editing) else {
            return;
        };
        let previous = self.config.templates.clone();
        let removed = self.config.templates.remove(index);
        if !self.save_config() {
            self.config.templates = previous;
            cx.notify();
            return;
        }
        self.templates.picked = 0;
        // Closed, so the toast's Undo (drawn under sheets) and ⌘Z reach it.
        self.templates.editor = None;
        let message = t("template.deleted").replace("{name}", &removed.name);
        self.push_undo(UndoAction::TemplateDeleted {
            index,
            template: removed,
        });
        self.show_undo_toast(message, cx);
    }

    /// Undo (`before = true`) puts a deleted template back at its place;
    /// redo deletes it again. The config is saved; a failed save restores
    /// the list and returns false (the banner says why). The editor is never
    /// open here: it absorbs ⌘Z and draws over the toast.
    pub(crate) fn replay_template_delete(
        &mut self,
        index: usize,
        template: &MetadataTemplate,
        before: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let previous = self.config.templates.clone();
        if before {
            // A template made since the delete may have taken the name.
            let taken = self
                .config
                .templates
                .iter()
                .any(|other| other.name.eq_ignore_ascii_case(&template.name));
            if taken {
                self.set_error(t("template.error_restore_name_taken"), cx);
                return false;
            }
            let at = index.min(self.config.templates.len());
            self.config.templates.insert(at, template.clone());
        } else if let Some(at) = self.config.templates.iter().position(|t| t == template) {
            self.config.templates.remove(at);
        } else {
            return true;
        }
        if !self.save_config() {
            self.config.templates = previous;
            cx.notify();
            return false;
        }
        self.templates.picked = 0;
        cx.notify();
        true
    }

    /// Apply Metadata Template…: the palette lists template names.
    pub fn start_template_pick(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = true;
        self.templates.palette_pick = true;
        self.command_query.clear();
        self.command_selected = 0;
        self.command_scroll.scroll_to_item(1);
        cx.notify();
    }

    /// Batch inspector picker: the next template, wrapping.
    pub fn cycle_picked_template(&mut self, cx: &mut Context<Self>) {
        let count = self.config.templates.len();
        if count > 0 {
            self.templates.picked = (self.templates.picked + 1) % count;
            cx.notify();
        }
    }

    /// Applies template `index` to the selection: one transaction, one undo
    /// entry, sidecars rewritten. A failure writes nothing and shows why.
    pub fn apply_template(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(template) = self.config.templates.get(index).cloned() else {
            return;
        };
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            self.set_error(t("template.error_no_selection"), cx);
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            return;
        };
        let change = match engine.apply_template(&ids, &template.rights, &template.keywords) {
            Ok(change) => change,
            Err(err) => {
                log::warn!("applying template {:?}: {err:?}", template.name);
                self.set_error(format!("{}: {err:#}", t("template.error_apply")), cx);
                return;
            }
        };
        self.push_undo(UndoAction::Template { change });
        self.sync_metadata_sidecars(&ids, cx);
        // Get Info caches rights per item; reload them.
        self.info_details = None;
        self.refresh_selected_tags();
        self.refresh_album_counts();
        let message = t(if ids.len() == 1 {
            "template.applied_one"
        } else {
            "template.applied_other"
        })
        .replace("{name}", &template.name)
        .replace("{n}", &ids.len().to_string());
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
        cx.notify();
    }
}
