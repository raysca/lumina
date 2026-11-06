//! Keyword hierarchy in the UI: the tree rows the tag manager and sidebar
//! draw, autocomplete over paths and aliases, the alias editor, and Move to….

use std::collections::{BTreeSet, HashMap};

use gpui::*;
use lumina_core::catalog::keywords::{leaf, parent_of};

use crate::app::types::TagSortOrder;
use crate::app::LuminaApp;
use crate::views::inspector::tabs::tags::{tag_suggestions, Suggestion};
use crate::views::sidebar::AlbumSelection;

/// One keyword row: a stored keyword, or an ancestor no item carries
/// directly (`count` is `None`; the tag manager loads its subtree count when
/// it is focused, never per frame).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct KeywordRow {
    pub path: String,
    pub depth: usize,
    pub count: Option<usize>,
}

impl KeywordRow {
    /// The last segment in the tree, the whole path in a flat list.
    pub(crate) fn label(&self) -> &str {
        if self.depth == 0 {
            &self.path
        } else {
            leaf(&self.path)
        }
    }
}

/// `tags` as a tree: sorted segment by segment (so `a/b` sits under `a`, not
/// after `a-z`), with every missing ancestor added as a row.
pub(crate) fn keyword_tree(tags: &[(String, usize)]) -> Vec<KeywordRow> {
    let counts: HashMap<&str, usize> = tags.iter().map(|(t, n)| (t.as_str(), *n)).collect();
    let mut paths: BTreeSet<Vec<&str>> = BTreeSet::new();
    for (tag, _) in tags {
        let segments: Vec<&str> = tag.split('/').collect();
        for end in 1..=segments.len() {
            paths.insert(segments[..end].to_vec());
        }
    }
    paths
        .into_iter()
        .map(|segments| {
            let path = segments.join("/");
            KeywordRow {
                depth: segments.len() - 1,
                count: counts.get(path.as_str()).copied(),
                path,
            }
        })
        .collect()
}

impl LuminaApp {
    /// Library tags after the manager's filter and sort, as painted in its
    /// list: A–Z with no filter is the keyword tree, otherwise a flat list of
    /// full paths.
    pub(crate) fn tag_manager_rows(&self) -> Vec<KeywordRow> {
        let query = self.tag_manager.search.to_lowercase();
        let mut rows: Vec<(String, usize)> = self
            .library_tag_counts
            .iter()
            .filter(|(tag, _)| query.is_empty() || tag.to_lowercase().contains(&query))
            .cloned()
            .collect();
        // A filtered list stays flat: a tree would add ancestors the filter
        // did not match, and ⌘A / ⌘⌫ would then delete their whole subtree.
        match self.tag_manager.sort {
            TagSortOrder::Alphabetical if query.is_empty() => return keyword_tree(&rows),
            TagSortOrder::Alphabetical => rows.sort_by(|a, b| a.0.split('/').cmp(b.0.split('/'))),
            TagSortOrder::CountDesc => {
                rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)))
            }
        }
        rows.into_iter()
            .map(|(path, count)| KeywordRow {
                path,
                depth: 0,
                count: Some(count),
            })
            .collect()
    }

    /// What the tag field offers for the typed text (also with the field
    /// closed, as the most used keywords), from cached counts and aliases.
    pub(crate) fn tag_input_suggestions(&self) -> Vec<Suggestion> {
        let typed = if self.tag_input_active {
            self.tag_input_buffer.as_str()
        } else {
            ""
        };
        // A selection offers what not every selected item has yet.
        let present: Vec<String> = if self.selected_indices.len() > 1 {
            self.selected_tag_counts
                .iter()
                .filter(|(_, n)| *n >= self.batch_summary.count)
                .map(|(tag, _)| tag.clone())
                .collect()
        } else {
            self.selected_item_tags.clone()
        };
        tag_suggestions(
            &present,
            &self.library_tag_counts,
            &self.keywords.aliases,
            typed,
        )
    }

    /// ↑/↓ in the tag field: walks the suggestions; ↑ from the first goes
    /// back to the typed text.
    pub(crate) fn step_tag_suggestion(&mut self, delta: isize, cx: &mut Context<Self>) {
        let len = self.tag_input_suggestions().len();
        self.keywords.suggestion_focus = match (self.keywords.suggestion_focus, delta < 0) {
            _ if len == 0 => None,
            (None, false) => Some(0),
            (None, true) => None,
            (Some(0), true) => None,
            (Some(i), true) => Some(i - 1),
            (Some(i), false) => Some((i + 1).min(len - 1)),
        };
        cx.notify();
    }

    /// The highlighted suggestion's keyword, which Enter adds instead of the
    /// typed text.
    pub(crate) fn focused_tag_suggestion(&self) -> Option<String> {
        let i = self.keywords.suggestion_focus?;
        self.tag_input_suggestions()
            .into_iter()
            .nth(i)
            .map(|s| s.path)
    }

    /// Reloads the alias rows; called with the tag counts and after alias
    /// edits, so rendering never queries the catalog.
    pub(crate) fn refresh_keyword_aliases(&mut self) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        match state.engine.aliases() {
            Ok(aliases) => self.keywords.aliases = aliases,
            Err(err) => log::warn!("could not load keyword aliases: {err:#}"),
        }
    }

    /// Aliases of `path`, A–Z.
    pub(crate) fn aliases_of(&self, path: &str) -> Vec<String> {
        self.keywords
            .aliases
            .iter()
            .filter(|(_, tag)| tag == path)
            .map(|(alias, _)| alias.clone())
            .collect()
    }

    /// Distinct items under the focused keyword, loaded once on focus.
    pub(crate) fn load_focused_keyword_count(&mut self) {
        self.tag_manager.focused_count = match (self.tag_manager.focused.as_ref(), &self.grid_state)
        {
            (Some(tag), Some(state)) => match state.engine.subtree_item_count(tag) {
                Ok(n) => n,
                Err(err) => {
                    log::warn!("could not count items under {tag:?}: {err:#}");
                    0
                }
            },
            _ => 0,
        };
    }

    /// The keyword the alias and move commands act on: the tag manager's
    /// focused row (⇧⌘A / ⇧⌘M and the detail buttons; the sheet absorbs ⌘K,
    /// so nothing else runs behind it), else the sidebar's tag view (palette).
    pub(crate) fn command_keyword(&self) -> Option<String> {
        if self.tag_manager.open {
            return self.tag_manager.focused.clone();
        }
        match &self.selection {
            AlbumSelection::Tag(tag) => Some(tag.clone()),
            _ => None,
        }
    }

    /// Shows the command's keyword in the tag manager, opening it if needed.
    fn focus_command_keyword(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(tag) = self.command_keyword() else {
            return false;
        };
        if !self.tag_manager.open {
            self.open_tag_manager(cx);
        }
        self.tag_manager.search_focused = false;
        self.focus_tag_manager_tag(tag, cx);
        true
    }

    /// Move to… (⇧⌘M, `MoveKeyword`): a rename whose prompt takes the new
    /// parent path; the keyword keeps its last segment.
    pub fn start_keyword_move(&mut self, cx: &mut Context<Self>) {
        if !self.focus_command_keyword(cx) {
            return;
        }
        let Some(tag) = self.tag_manager.focused.clone() else {
            return;
        };
        self.tag_manager.alias_editing = false;
        self.tag_manager.editing_buffer = parent_of(&tag).unwrap_or("").to_string();
        self.tag_manager.editing = Some(tag);
        self.tag_manager.moving = true;
        cx.notify();
    }

    /// The path a modal rename buffer names: the buffer itself, or for Move
    /// to… the buffer as parent plus the keyword's last segment (an empty
    /// parent moves it to the top level).
    pub(crate) fn modal_rename_target(&self, old_tag: &str, buffer: &str) -> String {
        if !self.tag_manager.moving {
            return buffer.to_string();
        }
        let parent = buffer.trim().trim_matches('/');
        if parent.is_empty() {
            leaf(old_tag).to_string()
        } else {
            format!("{parent}/{}", leaf(old_tag))
        }
    }

    /// Add Alias (⇧⌘A, `AddKeywordAlias`, or a click on the field).
    pub fn start_alias_input(&mut self, cx: &mut Context<Self>) {
        if !self.focus_command_keyword(cx) {
            return;
        }
        self.tag_manager.editing = None;
        self.tag_manager.moving = false;
        self.tag_manager.alias_editing = true;
        self.tag_manager.alias_buffer.clear();
        cx.notify();
    }

    pub fn cancel_alias_input(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.alias_editing = false;
        self.tag_manager.alias_buffer.clear();
        cx.notify();
    }

    /// Stores the typed alias for the focused keyword. A refusal (the alias
    /// is a keyword in use, or too long) goes to the error banner and keeps
    /// the field open for a correction.
    pub fn commit_alias_input(&mut self, cx: &mut Context<Self>) {
        let alias = self.tag_manager.alias_buffer.trim().to_string();
        let (Some(tag), Some(state)) = (self.tag_manager.focused.clone(), self.grid_state.as_ref())
        else {
            return;
        };
        if alias.is_empty() {
            self.cancel_alias_input(cx);
            return;
        }
        let result = state.engine.set_alias(&alias, &tag);
        if self
            .apply_tag_mutation("Could not add alias", result, cx)
            .is_none()
        {
            return;
        }
        self.refresh_keyword_aliases();
        self.cancel_alias_input(cx);
        self.show_toast(format!("\"{alias}\" now adds \"{tag}\""), cx);
    }

    pub fn remove_keyword_alias(&mut self, alias: String, cx: &mut Context<Self>) {
        let Some(state) = self.grid_state.as_ref() else {
            return;
        };
        let result = state.engine.remove_alias(&alias);
        if self
            .apply_tag_mutation("Could not remove alias", result, cx)
            .is_none()
        {
            return;
        }
        self.refresh_keyword_aliases();
        cx.notify();
    }

    /// ⌫ in the empty alias field removes the focused keyword's last alias,
    /// the keyboard path for its ×.
    pub(crate) fn remove_last_alias(&mut self, cx: &mut Context<Self>) {
        let last = self
            .tag_manager
            .focused
            .as_deref()
            .and_then(|tag| self.aliases_of(tag).pop());
        if let Some(alias) = last {
            self.remove_keyword_alias(alias, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::keyword_tree;

    #[core::prelude::v1::test]
    fn tree_sorts_by_segment_and_adds_missing_ancestors() {
        let tags: Vec<(String, usize)> = [("a-z", 1), ("a/b/c", 2), ("a", 3)]
            .iter()
            .map(|(t, n)| (t.to_string(), *n))
            .collect();
        let rows: Vec<(String, usize, Option<usize>)> = keyword_tree(&tags)
            .into_iter()
            .map(|r| (r.path, r.depth, r.count))
            .collect();
        assert_eq!(
            rows,
            [
                ("a".into(), 0, Some(3)),
                ("a/b".into(), 1, None),
                ("a/b/c".into(), 2, Some(2)),
                ("a-z".into(), 0, Some(1)),
            ],
            "children follow their parent; a missing ancestor is a row without a count"
        );
    }
}
