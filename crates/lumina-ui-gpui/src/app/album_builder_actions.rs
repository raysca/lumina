use gpui::*;
use lumina_core::{MatchMode, RuleField, RuleOperator, RuleValue, SmartAlbumRule};

use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

impl LuminaApp {
    pub fn open_album_builder(&mut self, cx: &mut Context<Self>) {
        self.album_draft.modal_open = true;
        self.album_draft.album_title = "My Smart Album".to_string();
        self.album_draft.title_focused = false;
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.logic_dropdown_open = false;
        self.album_draft.field_dropdown = None;
        self.album_draft.operator_dropdown = None;
        self.album_draft.value_dropdown = None;
        self.album_draft.rule_value_editing = None;
        self.album_draft.rule_value_buffer.clear();
        self.album_draft.rules = vec![SmartAlbumRule {
            field: RuleField::IsFavorite,
            operator: RuleOperator::Equals,
            value: RuleValue::Bool(true),
        }];
        self.update_draft_count();
        cx.notify();
    }

    pub fn close_album_builder(&mut self, cx: &mut Context<Self>) {
        self.album_draft.modal_open = false;
        self.album_draft.title_focused = false;
        self.album_draft.logic_dropdown_open = false;
        self.album_draft.field_dropdown = None;
        self.album_draft.operator_dropdown = None;
        self.album_draft.value_dropdown = None;
        self.album_draft.rule_value_editing = None;
        self.album_draft.rule_value_buffer.clear();
        cx.notify();
    }

    pub fn focus_draft_title(&mut self, cx: &mut Context<Self>) {
        self.album_draft.title_focused = true;
        self.album_draft.logic_dropdown_open = false;
        self.album_draft.field_dropdown = None;
        self.album_draft.operator_dropdown = None;
        self.album_draft.value_dropdown = None;
        self.album_draft.rule_value_editing = None;
        cx.notify();
    }

    #[cfg(test)]
    pub fn unfocus_draft_title(&mut self, cx: &mut Context<Self>) {
        self.album_draft.title_focused = false;
        cx.notify();
    }

    #[cfg(test)]
    pub fn toggle_draft_logic_dropdown(&mut self, cx: &mut Context<Self>) {
        self.album_draft.logic_dropdown_open = !self.album_draft.logic_dropdown_open;
        self.album_draft.field_dropdown = None;
        self.album_draft.operator_dropdown = None;
        self.album_draft.value_dropdown = None;
        self.album_draft.title_focused = false;
        self.album_draft.rule_value_editing = None;
        cx.notify();
    }

    pub fn select_draft_logic(&mut self, mode: MatchMode, cx: &mut Context<Self>) {
        self.album_draft.match_mode = mode;
        self.album_draft.logic_dropdown_open = false;
        self.update_draft_count();
        cx.notify();
    }

    pub fn toggle_draft_field_dropdown(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.album_draft.field_dropdown == Some(index) {
            self.album_draft.field_dropdown = None;
        } else {
            self.album_draft.field_dropdown = Some(index);
            self.album_draft.operator_dropdown = None;
            self.album_draft.value_dropdown = None;
            self.album_draft.logic_dropdown_open = false;
            self.album_draft.title_focused = false;
            self.album_draft.rule_value_editing = None;
        }
        cx.notify();
    }

    pub fn set_draft_rule_field(&mut self, index: usize, field: RuleField, cx: &mut Context<Self>) {
        if index < self.album_draft.rules.len() {
            let (op, val) = match field {
                RuleField::IsFavorite => (RuleOperator::Equals, RuleValue::Bool(true)),
                RuleField::MediaType => {
                    (RuleOperator::Equals, RuleValue::Text("video".to_string()))
                }
                RuleField::Tag => (RuleOperator::Equals, RuleValue::Text(String::new())),
                RuleField::CameraMake => {
                    (RuleOperator::Contains, RuleValue::Text("Sony".to_string()))
                }
                RuleField::CameraModel => (RuleOperator::Contains, RuleValue::Text(String::new())),
                RuleField::Lens => (RuleOperator::Contains, RuleValue::Text(String::new())),
                RuleField::City => (RuleOperator::Contains, RuleValue::Text(String::new())),
                RuleField::Year => (RuleOperator::Equals, RuleValue::Integer(2026)),
                RuleField::Iso => (RuleOperator::GreaterThan, RuleValue::Integer(1600)),
                RuleField::FNumber => (RuleOperator::LessThan, RuleValue::Number(2.8)),
                RuleField::Rating => (RuleOperator::GreaterThan, RuleValue::Integer(3)),
                RuleField::ColorLabel => (RuleOperator::Equals, RuleValue::Integer(1)),
                _ => (RuleOperator::Equals, RuleValue::Text(String::new())),
            };
            self.album_draft.rules[index] = SmartAlbumRule {
                field,
                operator: op,
                value: val,
            };
            self.album_draft.field_dropdown = None;
            self.update_draft_count();
            cx.notify();
        }
    }

    pub fn toggle_draft_operator_dropdown(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.album_draft.operator_dropdown == Some(index) {
            self.album_draft.operator_dropdown = None;
        } else {
            self.album_draft.operator_dropdown = Some(index);
            self.album_draft.field_dropdown = None;
            self.album_draft.value_dropdown = None;
            self.album_draft.logic_dropdown_open = false;
            self.album_draft.title_focused = false;
            self.album_draft.rule_value_editing = None;
        }
        cx.notify();
    }

    pub fn set_draft_rule_operator(
        &mut self,
        index: usize,
        op: RuleOperator,
        cx: &mut Context<Self>,
    ) {
        if index < self.album_draft.rules.len() {
            self.album_draft.rules[index].operator = op;
            self.album_draft.operator_dropdown = None;
            self.update_draft_count();
            cx.notify();
        }
    }

    pub fn toggle_draft_value_dropdown(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.album_draft.value_dropdown == Some(index) {
            self.album_draft.value_dropdown = None;
        } else {
            self.album_draft.value_dropdown = Some(index);
            self.album_draft.field_dropdown = None;
            self.album_draft.operator_dropdown = None;
            self.album_draft.logic_dropdown_open = false;
            self.album_draft.title_focused = false;
            self.album_draft.rule_value_editing = None;
        }
        cx.notify();
    }

    pub fn set_draft_rule_photo_choice(
        &mut self,
        index: usize,
        choice: &str,
        cx: &mut Context<Self>,
    ) {
        if index < self.album_draft.rules.len() {
            match choice {
                "favourite" => {
                    self.album_draft.rules[index].field = RuleField::IsFavorite;
                    self.album_draft.rules[index].value = RuleValue::Bool(true);
                }
                "not favourite" => {
                    self.album_draft.rules[index].field = RuleField::IsFavorite;
                    self.album_draft.rules[index].value = RuleValue::Bool(false);
                }
                "video" => {
                    self.album_draft.rules[index].field = RuleField::MediaType;
                    self.album_draft.rules[index].value = RuleValue::Text("video".to_string());
                }
                "raw" => {
                    self.album_draft.rules[index].field = RuleField::MediaType;
                    self.album_draft.rules[index].value = RuleValue::Text("raw".to_string());
                }
                "still photo" => {
                    self.album_draft.rules[index].field = RuleField::MediaType;
                    self.album_draft.rules[index].value = RuleValue::Text("image".to_string());
                }
                _ => {}
            }
            self.album_draft.value_dropdown = None;
            self.update_draft_count();
            cx.notify();
        }
    }

    pub fn start_editing_rule_value(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.album_draft.rules.len() {
            self.album_draft.rule_value_editing = Some(index);
            self.album_draft.rule_value_buffer = match &self.album_draft.rules[index].value {
                RuleValue::Text(s) => s.clone(),
                RuleValue::Number(n) => format!("{:.1}", n),
                RuleValue::Integer(i) => format!("{}", i),
                RuleValue::Bool(b) => format!("{}", b),
                RuleValue::Range(min, max) => format!("{} - {}", min, max),
            };
            self.album_draft.title_focused = false;
            self.album_draft.logic_dropdown_open = false;
            self.album_draft.field_dropdown = None;
            self.album_draft.operator_dropdown = None;
            self.album_draft.value_dropdown = None;
            cx.notify();
        }
    }

    pub fn commit_editing_rule_value(&mut self, cx: &mut Context<Self>) {
        if let Some(index) = self.album_draft.rule_value_editing.take() {
            if index < self.album_draft.rules.len() {
                let buf = self.album_draft.rule_value_buffer.trim();
                let rule = &mut self.album_draft.rules[index];
                match rule.field {
                    RuleField::Iso
                    | RuleField::Year
                    | RuleField::Month
                    | RuleField::Rating
                    | RuleField::ColorLabel => {
                        let num = buf.parse::<i64>().unwrap_or(0);
                        rule.value = RuleValue::Integer(num);
                    }
                    RuleField::FNumber => {
                        let num = buf.parse::<f64>().unwrap_or(0.0);
                        rule.value = RuleValue::Number(num);
                    }
                    _ => {
                        rule.value = RuleValue::Text(buf.to_string());
                    }
                }
            }
            self.update_draft_count();
        }
        cx.notify();
    }

    pub fn cancel_editing_rule_value(&mut self, cx: &mut Context<Self>) {
        self.album_draft.rule_value_editing = None;
        self.album_draft.rule_value_buffer.clear();
        cx.notify();
    }

    pub fn insert_draft_rule_after(&mut self, index: usize, cx: &mut Context<Self>) {
        let insert_idx = if index < self.album_draft.rules.len() {
            index + 1
        } else {
            self.album_draft.rules.len()
        };
        self.album_draft.rules.insert(
            insert_idx,
            SmartAlbumRule {
                field: RuleField::IsFavorite,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            },
        );
        self.update_draft_count();
        cx.notify();
    }

    pub fn update_draft_count(&mut self) {
        if let Some(ref state) = self.grid_state {
            let draft = lumina_core::SmartAlbum::new(
                self.album_draft.album_title.clone(),
                "folder".to_string(),
                self.album_draft.match_mode,
                self.album_draft.rules.clone(),
            );
            self.album_draft.match_count = state.engine.query_smart_album_count(&draft);
        }
    }

    pub fn remove_draft_rule(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.album_draft.rules.len() > 1 && index < self.album_draft.rules.len() {
            self.album_draft.rules.remove(index);
            self.update_draft_count();
            cx.notify();
        }
    }

    pub fn apply_template_untagged(&mut self, cx: &mut Context<Self>) {
        self.album_draft.album_title = "Untagged Photos".to_string();
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.rules = vec![SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::IsEmpty,
            value: RuleValue::Text(String::new()),
        }];
        self.update_draft_count();
        cx.notify();
    }

    pub fn apply_template_tagged(&mut self, cx: &mut Context<Self>) {
        self.album_draft.album_title = "Tagged Media".to_string();
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.rules = vec![SmartAlbumRule {
            field: RuleField::Tag,
            operator: RuleOperator::IsNotEmpty,
            value: RuleValue::Text(String::new()),
        }];
        self.update_draft_count();
        cx.notify();
    }

    pub fn apply_template_sony_favorites(&mut self, cx: &mut Context<Self>) {
        self.album_draft.album_title = "Sony Favorites".to_string();
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.rules = vec![
            SmartAlbumRule {
                field: RuleField::IsFavorite,
                operator: RuleOperator::Equals,
                value: RuleValue::Bool(true),
            },
            SmartAlbumRule {
                field: RuleField::CameraMake,
                operator: RuleOperator::Contains,
                value: RuleValue::Text("Sony".to_string()),
            },
        ];
        self.update_draft_count();
        cx.notify();
    }

    pub fn apply_template_fast_apertures(&mut self, cx: &mut Context<Self>) {
        self.album_draft.album_title = "Fast Glass (f < 2.8)".to_string();
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.rules = vec![SmartAlbumRule {
            field: RuleField::FNumber,
            operator: RuleOperator::LessThan,
            value: RuleValue::Number(2.8),
        }];
        self.update_draft_count();
        cx.notify();
    }

    pub fn apply_template_high_iso(&mut self, cx: &mut Context<Self>) {
        self.album_draft.album_title = "Low Light (ISO > 1600)".to_string();
        self.album_draft.match_mode = MatchMode::All;
        self.album_draft.rules = vec![SmartAlbumRule {
            field: RuleField::Iso,
            operator: RuleOperator::GreaterThan,
            value: RuleValue::Integer(1600),
        }];
        self.update_draft_count();
        cx.notify();
    }

    pub fn save_draft_smart_album(&mut self, cx: &mut Context<Self>) {
        let title = if self.album_draft.album_title.trim().is_empty() {
            "Custom Smart Album".to_string()
        } else {
            self.album_draft.album_title.trim().to_string()
        };
        let created_id = if let Some(ref state) = self.grid_state {
            state
                .engine
                .create_smart_album(
                    &title,
                    "folder",
                    self.album_draft.match_mode,
                    &self.album_draft.rules,
                )
                .ok()
                .map(|a| a.id)
        } else {
            None
        };

        if let Some(album_id) = created_id {
            self.album_draft.modal_open = false;
            self.album_draft.title_focused = false;
            self.album_draft.logic_dropdown_open = false;
            self.album_draft.field_dropdown = None;
            self.album_draft.operator_dropdown = None;
            self.album_draft.value_dropdown = None;
            self.album_draft.rule_value_editing = None;
            self.album_draft.rule_value_buffer.clear();
            self.selection = AlbumSelection::User(album_id);
            self.selected_index = None;
            self.selected_item = None;
            self.refresh_album_counts();
            cx.notify();
        }
    }

    pub fn toggle_draft_show_in_sidebar(&mut self, cx: &mut Context<Self>) {
        self.album_draft.show_in_sidebar = !self.album_draft.show_in_sidebar;
        cx.notify();
    }
}
