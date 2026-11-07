//! Keeps the selection on the same items, by id, across the app's own
//! metadata writes.
//!
//! A flag in Show Rejected or a rating in a `rating:` search can drop rows
//! from the view or reorder it. After the write the view re-reads
//! (`active_view.rs`, `navigable`), so the old positions name other items
//! and a following key would act on them (AGENTS 5.3). Writers take a
//! [`HeldSelection`] before the write and hand it to
//! [`LuminaApp::reselect_held`] after it.
//!
//! A write that changes `k` items moves every other row by at most `k`
//! positions, so only the held span widened by `k` is re-read: one bounded
//! slice, never a walk of the view (the UI thread must not scan 100k rows).

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::app::undo::UndoAction;

use gpui::*;
use lumina_core::MediaId;

use crate::app::LuminaApp;

/// The selection by identity, taken before a write.
pub(crate) struct HeldSelection {
    ids: Vec<MediaId>,
    /// The shown/focused item.
    focus: Option<MediaId>,
    anchor: Option<MediaId>,
    /// Lowest and highest selected positions before the write.
    span: Option<(usize, usize)>,
    /// How many items the write changes: how far any row can move.
    moved: usize,
    /// The focused item sat at `selected_index`. When not (no library, or
    /// an item shown from outside the view), positions were never this
    /// view's and are left alone.
    in_view: bool,
}

impl HeldSelection {
    /// Another write of `n` items before [`LuminaApp::reselect_held`]: rows
    /// may now be `n` further from where they were held.
    pub(crate) fn widen(&mut self, n: usize) {
        self.moved += n;
    }

    /// Rows `lo..=hi` of a view of `count` rows that can hold the held
    /// items, or `None` when the selection was never this view's or the
    /// view is empty.
    pub(crate) fn window(&self, count: usize) -> Option<(usize, usize)> {
        let (lo, hi) = self.span.filter(|_| self.in_view && count > 0)?;
        let last = count - 1;
        Some((
            lo.saturating_sub(self.moved).min(last),
            (hi + self.moved).min(last),
        ))
    }
}

impl LuminaApp {
    /// The selection (`ids`, from `get_selected_ids`) by identity, before a
    /// write that changes `moved` items, while positions are still valid.
    pub(crate) fn hold_selection(&self, ids: &[MediaId], moved: usize) -> HeldSelection {
        let focus = self.selected_item.as_ref().map(|item| item.id);
        HeldSelection {
            ids: ids.to_vec(),
            focus,
            span: self
                .selected_indices
                .iter()
                .copied()
                .chain(self.selected_index)
                .fold(None, |range: Option<(usize, usize)>, i| match range {
                    Some((lo, hi)) => Some((lo.min(i), hi.max(i))),
                    None => Some((i, i)),
                }),
            moved,
            in_view: focus.is_some()
                && self
                    .selected_index
                    .and_then(|index| self.navigation_item(index))
                    .map(|item| item.id)
                    == focus,
            anchor: self
                .selection_anchor
                .and_then(|index| self.navigation_item(index))
                .map(|item| item.id),
        }
    }

    /// [`Self::hold_selection`] for an undo/redo step that rewrites flags,
    /// ratings or colour labels, which can move rows the same way.
    pub(crate) fn hold_for_history(&self, action: &UndoAction) -> Option<HeldSelection> {
        let moved = match action {
            UndoAction::Flag { changes } => changes.len(),
            UndoAction::Rating { changes } | UndoAction::ColorLabel { changes } => changes.len(),
            _ => return None,
        };
        Some(self.hold_selection(&self.get_selected_ids(), moved))
    }

    /// Puts the selection back on `held`'s items at their positions in the
    /// re-read view. Items that left the view leave the selection. When all
    /// of them left: the grid selects nothing (a next key must not reach an
    /// unselected item); the viewer shows the row now at the old position,
    /// as trash does, and closes when the view is empty. So a palette or
    /// context-menu flag in the viewer that drops the shown item from the
    /// view moves to the next row, as trash does; keys advance by id in
    /// `culling.rs` instead.
    /// ponytail: one inexact case. A rating while sorted by Rating can move
    /// the rated item further than `moved` rows, out of the window: in the
    /// grid it drops from the selection (safe: nothing else is selected),
    /// and in the viewer a palette or context-menu rating shows the row now
    /// at the old position. A core position-of-id query is the upgrade.
    pub(crate) fn reselect_held(&mut self, held: HeldSelection, cx: &mut Context<Self>) {
        let (true, Some((lo, hi))) = (held.in_view, held.span) else {
            return;
        };
        let positions =
            self.positions_in(&held.ids, lo.saturating_sub(held.moved), hi + held.moved);
        if positions.is_empty() {
            if !self.is_fullscreen_viewer {
                self.clear_selection(cx);
                return;
            }
            let count = self.navigation_count();
            if count == 0 {
                self.close_viewer(cx);
                self.clear_selection(cx);
            } else {
                self.select_index(self.selected_index.unwrap_or(0).min(count - 1), cx);
            }
            return;
        }
        let at = |id: Option<MediaId>| id.and_then(|id| positions.get(&id).copied());
        let focus_kept = at(held.focus);
        let Some(focus) = focus_kept.or_else(|| positions.values().min().copied()) else {
            return;
        };
        let indices: BTreeSet<usize> = positions.values().copied().collect();
        if indices == self.selected_indices && self.selected_index == Some(focus) {
            return;
        }
        self.selected_indices = indices;
        self.selected_index = Some(focus);
        self.selection_anchor = at(held.anchor).or(Some(focus));
        // The shown item only changes when it left the view.
        if focus_kept.is_none() {
            if let Some(item) = self.navigation_item(focus) {
                self.select_item_internal(item, cx);
            }
        }
        self.update_batch_summary();
        self.reveal_grid_selection();
        cx.notify();
    }

    /// Where each of `ids` sits among rows `lo..=hi` of the current view
    /// (one bounded read); ids outside that window are left out.
    pub(crate) fn positions_in(
        &self,
        ids: &[MediaId],
        lo: usize,
        hi: usize,
    ) -> HashMap<MediaId, usize> {
        let wanted: HashSet<MediaId> = ids.iter().copied().collect();
        let mut found = HashMap::new();
        if !wanted.is_empty() {
            note(&mut found, &wanted, lo, self.navigation_rows(lo..hi + 1));
        }
        found
    }
}

/// Records the position of each wanted id among `rows`, read from `offset`.
fn note(
    found: &mut HashMap<MediaId, usize>,
    wanted: &HashSet<MediaId>,
    offset: usize,
    rows: Vec<lumina_core::MediaItem>,
) {
    for (i, item) in rows.into_iter().enumerate() {
        if wanted.contains(&item.id) {
            found.insert(item.id, offset + i);
        }
    }
}
