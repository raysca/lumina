//! Undo entries for global keyword rename, merge and delete: recording one
//! (with where the view pointed) and pointing the view back on replay.

use gpui::*;
use lumina_core::catalog::keywords::{normalize_path, subtree_suffix};
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

/// One replay of a keyword undo entry, for
/// [`LuminaApp::restore_tag_view_refs`].
pub(crate) struct TagReplay<'a> {
    /// The view the replayed side left, and the one to restore.
    pub from: &'a TagViewRefs,
    pub to: &'a TagViewRefs,
    pub keeps_positions: bool,
    /// `(old, new)` paths the op moved; see [`LuminaApp::push_tag_rows_undo`].
    pub moves: &'a [(String, String)],
    /// Undo (true) or redo.
    pub undo: bool,
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
    /// `moves` lists each `(old, new)` keyword path the op moved (a rename
    /// or move, or each merged source and its target), so a keyword view
    /// opened after the op can follow a replay.
    ///
    /// Returns false when it changed nothing, so the caller shows no Undo
    /// (it would undo an older, unrelated action).
    pub(crate) fn push_tag_rows_undo(
        &mut self,
        label: &'static str,
        change: TagChange,
        view_before: TagViewRefs,
        keeps_positions: bool,
        moves: Vec<(String, String)>,
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
            moves,
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
    /// replay left) at `to`. A keyword view opened since (the op ran on the
    /// Tags page, say) follows `moves` instead, and one left on a keyword
    /// that no longer exists falls back to All Media. Grid positions survive
    /// only when `keeps_positions` (see [`Self::push_tag_rows_undo`]).
    pub(crate) fn restore_tag_view_refs(&mut self, replay: TagReplay<'_>, cx: &mut Context<Self>) {
        let TagReplay {
            from,
            to,
            keeps_positions,
            moves,
            undo,
        } = replay;
        // A replay always changes rows, so positions go unless it is a rename.
        if !keeps_positions {
            self.clear_selection_fields();
        }
        if self.selection == from.selection && from.selection != to.selection {
            self.selection = to.selection.clone();
        } else if let AlbumSelection::Tag(current) = &self.selection {
            let current = current.to_lowercase();
            // Undo maps new → old only for a pure rename (the new path is
            // gone again); after a merge undo the target still exists and
            // the view stays on it. Redo always maps old → new.
            let mapped =
                moves
                    .iter()
                    .filter(|_| !undo || keeps_positions)
                    .find_map(|(old, new)| {
                        let (gone, kept) = if undo { (new, old) } else { (old, new) };
                        subtree_suffix(&current, gone).map(|rest| format!("{kept}{rest}"))
                    });
            let path = mapped.unwrap_or(current);
            self.selection = if self.keyword_is_unused(&path) {
                self.clear_selection_fields();
                AlbumSelection::AllLibrary
            } else {
                AlbumSelection::Tag(path)
            };
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
