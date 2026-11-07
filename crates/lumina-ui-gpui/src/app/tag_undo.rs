//! Undo entries for global keyword rename, merge and delete: recording one
//! (with where the view pointed) and pointing the view back on replay.

use gpui::*;
use lumina_core::catalog::keywords::normalize_path;
use lumina_core::TagChange;

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::views::sidebar::AlbumSelection;

/// Where the sidebar selection, a `tag:` search and the tag manager's focus
/// pointed, so undoing a keyword rename, merge or delete can point them
/// back (the op moved them with `retarget_tag_refs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagViewRefs {
    pub selection: AlbumSelection,
    pub search_query: String,
    pub focused: Option<String>,
}

impl LuminaApp {
    /// True when no active item carries `path` or a keyword below it, so a
    /// rename onto it is a pure rename (the tag view keeps the same items)
    /// rather than a merge. A failed count says false (the safe answer).
    pub(crate) fn keyword_is_unused(&self, path: &str) -> bool {
        let Some(state) = self.grid_state.as_ref() else {
            return false;
        };
        let path = normalize_path(path).unwrap_or_else(|_| path.to_string());
        matches!(state.engine.subtree_item_count(&path), Ok(0))
    }

    /// Records a global keyword change as one undo entry, with the view
    /// as it was before the op (`view_before`) and as it is now (call it
    /// after `retarget_tag_refs`). `keeps_positions` is true only for a
    /// rename or move onto an unused path; any other change can alter what
    /// the current view (a tag, its parent, a `tag:` search) shows, so the
    /// grid positions are cleared.
    ///
    /// Returns false when it changed nothing, so the caller shows no Undo
    /// (it would undo an older, unrelated action).
    pub(crate) fn push_tag_rows_undo(
        &mut self,
        label: &'static str,
        change: TagChange,
        view_before: TagViewRefs,
        keeps_positions: bool,
    ) -> bool {
        // ponytail: clears even when this view's items did not change;
        // safe, and cheaper than diffing the item set.
        if !keeps_positions
            && (self.selection != view_before.selection || change != TagChange::default())
        {
            self.clear_selection_fields();
        }
        if change == TagChange::default() {
            return false;
        }
        let view_after = self.tag_view_refs();
        self.push_undo(UndoAction::TagRows {
            label,
            change,
            view_before,
            view_after,
            keeps_positions,
        });
        true
    }

    pub(crate) fn tag_view_refs(&self) -> TagViewRefs {
        TagViewRefs {
            selection: self.selection.clone(),
            search_query: self.search_query.clone(),
            focused: self.tag_manager.focused.clone(),
        }
    }

    /// Points each reference that still shows `from` (what the op or its
    /// replay left) at `to`; one the user moved since is left alone. Grid
    /// positions survive only when `keeps_positions` (see
    /// [`Self::push_tag_rows_undo`]).
    pub(crate) fn restore_tag_view_refs(
        &mut self,
        from: &TagViewRefs,
        to: &TagViewRefs,
        keeps_positions: bool,
        cx: &mut Context<Self>,
    ) {
        // A replay always changes rows, so positions go unless it is a rename.
        if !keeps_positions {
            self.clear_selection_fields();
        }
        if self.selection == from.selection && from.selection != to.selection {
            self.selection = to.selection.clone();
        }
        if self.search_query == from.search_query && from.search_query != to.search_query {
            if to.search_query.is_empty() {
                self.clear_search(cx);
            } else {
                self.search_query = to.search_query.clone();
            }
        }
        if self.tag_manager.focused == from.focused {
            self.tag_manager.focused = to.focused.clone();
        }
    }

    /// The toast after a global keyword change: with Undo when one was
    /// recorded.
    pub(crate) fn tag_rows_toast(
        &mut self,
        recorded: bool,
        message: String,
        cx: &mut Context<Self>,
    ) {
        if recorded {
            self.show_undo_toast(message, cx);
        } else {
            self.show_toast(message, cx);
        }
    }
}
