use gpui::*;
use lumina_core::{RuleField, RuleValue};

use crate::app::LuminaApp;

impl LuminaApp {
    /// Handles keystrokes when any modal dialog or inline text field is active.
    /// Absorbs keystrokes so background shortcuts don't mutate or delete files.
    /// Returns `true` if a modal consumed the event, `false` otherwise.
    pub fn handle_modal_key_down(
        &mut self,
        key: &str,
        cmd_or_ctrl: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        // 0. Keyboard shortcut cheat sheet
        if self.shortcuts_overlay_open {
            if key == "escape" || key == "/" {
                self.close_shortcuts_overlay(cx);
            }
            return true;
        }

        // 0b. Preferences modal
        if self.preferences_open {
            if key == "escape" {
                self.close_preferences(cx);
            }
            return true;
        }

        // 1. Delete confirmation modal
        if self.delete_confirm_state.is_some() {
            if key == "escape" {
                self.close_delete_modal(cx);
            } else if key == "enter" || key == "return" {
                self.execute_confirmed_deletion(cx);
            }
            return true;
        }

        // 2. Smart album builder modal
        if self.album_draft.modal_open {
            if let Some(index) = self.album_draft.rule_value_editing {
                if key == "escape" {
                    self.cancel_editing_rule_value(cx);
                    return true;
                }
                if key == "enter" || key == "return" {
                    self.commit_editing_rule_value(cx);
                    return true;
                }
                if key == "backspace" {
                    self.album_draft.rule_value_buffer.pop();
                    let buf = self.album_draft.rule_value_buffer.clone();
                    if index < self.album_draft.rules.len() {
                        let rule = &mut self.album_draft.rules[index];
                        match rule.field {
                            RuleField::Iso | RuleField::Year | RuleField::Month => {
                                let num = buf.parse::<i64>().unwrap_or(0);
                                rule.value = RuleValue::Integer(num);
                            }
                            RuleField::FNumber => {
                                let num = buf.parse::<f64>().unwrap_or(0.0);
                                rule.value = RuleValue::Number(num);
                            }
                            _ => {
                                rule.value = RuleValue::Text(buf);
                            }
                        }
                    }
                    self.update_draft_count();
                    cx.notify();
                    return true;
                }
                if !cmd_or_ctrl {
                    if key == "space" {
                        self.album_draft.rule_value_buffer.push(' ');
                        let buf = self.album_draft.rule_value_buffer.clone();
                        if index < self.album_draft.rules.len() {
                            self.album_draft.rules[index].value = RuleValue::Text(buf);
                        }
                        self.update_draft_count();
                        cx.notify();
                        return true;
                    } else if key.chars().count() == 1
                        && !key.chars().next().is_some_and(|c| c.is_control())
                    {
                        self.album_draft.rule_value_buffer.push_str(key);
                        let buf = self.album_draft.rule_value_buffer.clone();
                        if index < self.album_draft.rules.len() {
                            let rule = &mut self.album_draft.rules[index];
                            match rule.field {
                                RuleField::Iso | RuleField::Year | RuleField::Month => {
                                    let num = buf.parse::<i64>().unwrap_or(0);
                                    rule.value = RuleValue::Integer(num);
                                }
                                RuleField::FNumber => {
                                    let num = buf.parse::<f64>().unwrap_or(0.0);
                                    rule.value = RuleValue::Number(num);
                                }
                                _ => {
                                    rule.value = RuleValue::Text(buf);
                                }
                            }
                        }
                        self.update_draft_count();
                        cx.notify();
                        return true;
                    }
                }
                return true;
            }

            if self.album_draft.title_focused {
                if key == "escape" {
                    self.album_draft.title_focused = false;
                    cx.notify();
                    return true;
                }
                if key == "enter" || key == "return" {
                    self.album_draft.title_focused = false;
                    cx.notify();
                    return true;
                }
                if key == "backspace" {
                    self.album_draft.album_title.pop();
                    self.update_draft_count();
                    cx.notify();
                    return true;
                }
                if !cmd_or_ctrl {
                    if key == "space" {
                        self.album_draft.album_title.push(' ');
                        self.update_draft_count();
                        cx.notify();
                        return true;
                    } else if key.chars().count() == 1
                        && !key.chars().next().is_some_and(|c| c.is_control())
                    {
                        self.album_draft.album_title.push_str(key);
                        self.update_draft_count();
                        cx.notify();
                        return true;
                    }
                }
                return true;
            }

            if (self.album_draft.logic_dropdown_open
                || self.album_draft.field_dropdown.is_some()
                || self.album_draft.operator_dropdown.is_some()
                || self.album_draft.value_dropdown.is_some())
                && key == "escape"
            {
                self.album_draft.logic_dropdown_open = false;
                self.album_draft.field_dropdown = None;
                self.album_draft.operator_dropdown = None;
                self.album_draft.value_dropdown = None;
                cx.notify();
                return true;
            }

            if key == "escape" {
                self.close_album_builder(cx);
            } else if key == "enter" || key == "return" {
                self.save_draft_smart_album(cx);
            }
            return true;
        }

        // 3. Inline tag input in inspector
        if self.tag_input_active {
            if key == "escape" {
                self.tag_input_active = false;
                self.tag_input_buffer.clear();
                cx.notify();
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_tag_input(cx);
                return true;
            }
            if key == "backspace" {
                self.tag_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.tag_input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.tag_input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 3a. Inline title input in inspector
        if self.caption.title_input_active {
            if key == "escape" {
                self.cancel_title_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_title_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.title_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.caption.title_input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.caption.title_input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 3b. Inline date input in inspector
        if self.caption.date_input_active {
            if key == "escape" {
                self.cancel_date_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_date_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.date_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl && key.chars().count() == 1 {
                self.caption.date_input_buffer.push_str(key);
                cx.notify();
                return true;
            }
            return true;
        }

        // 3c. Inline location input in inspector
        if self.caption.location_input_active {
            if key == "escape" {
                self.cancel_location_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_location_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.location_input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl && key.chars().count() == 1 {
                self.caption.location_input_buffer.push_str(key);
                cx.notify();
                return true;
            }
            return true;
        }

        // 3d. Inline caption input in inspector
        if self.caption.input_active {
            if key == "escape" {
                self.cancel_caption_input(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_caption_input(cx);
                return true;
            }
            if key == "backspace" {
                self.caption.input_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.caption.input_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.caption.input_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 4. Editing / renaming tag in inspector
        if self.editing_tag.is_some() {
            if key == "escape" {
                self.cancel_tag_rename(cx);
                return true;
            }
            if key == "enter" || key == "return" {
                self.commit_tag_rename(cx);
                return true;
            }
            if key == "backspace" {
                self.editing_tag_buffer.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.editing_tag_buffer.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.editing_tag_buffer.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 5. Tag manager modal
        if self.tag_manager.open {
            if self.tag_manager.editing.is_some() {
                if key == "escape" {
                    self.cancel_modal_tag_rename(cx);
                    return true;
                }
                if key == "enter" || key == "return" {
                    self.commit_modal_tag_rename(cx);
                    return true;
                }
                if key == "backspace" {
                    self.tag_manager.editing_buffer.pop();
                    cx.notify();
                    return true;
                }
                if !cmd_or_ctrl {
                    if key == "space" {
                        self.tag_manager.editing_buffer.push(' ');
                        cx.notify();
                        return true;
                    } else if key.chars().count() == 1
                        && !key.chars().next().is_some_and(|c| c.is_control())
                    {
                        self.tag_manager.editing_buffer.push_str(key);
                        cx.notify();
                        return true;
                    }
                }
                return true;
            }

            if key == "escape" {
                if self.tag_manager.merge_source.is_some() {
                    self.cancel_tag_merge(cx);
                    return true;
                }
                self.close_tag_manager(cx);
                return true;
            }

            // When in Tag Manager without active rename, typing filters tags
            if key == "backspace" {
                self.tag_manager.search.pop();
                cx.notify();
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.tag_manager.search.push(' ');
                    cx.notify();
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.tag_manager.search.push_str(key);
                    cx.notify();
                    return true;
                }
            }
            return true;
        }

        // 6. Toolbar search bar input focus
        if self.search_focused {
            if key == "escape" || key == "enter" || key == "return" {
                self.search_focused = false;
                cx.notify();
                return true;
            }
            if key == "backspace" {
                self.search_query.pop();
                self.clear_selection(cx);
                return true;
            }
            if !cmd_or_ctrl {
                if key == "space" {
                    self.search_query.push(' ');
                    self.clear_selection(cx);
                    return true;
                } else if key.chars().count() == 1
                    && !key.chars().next().is_some_and(|c| c.is_control())
                {
                    self.search_query.push_str(key);
                    self.clear_selection(cx);
                    return true;
                }
            }
            return true;
        }

        false
    }
}
