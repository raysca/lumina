//! Similar shots: the Duplicates destination's second tab, listing the
//! RAW+JPEG pairs and bursts that `LuminaEngine::find_stacks` computes. Its
//! two actions only flag, through [`LuminaApp::set_flag_on`] (one write,
//! one undo entry, the banner on failure); the rejects are deleted, if at
//! all, by Delete Rejected….

use gpui::*;
use std::collections::HashMap;

use lumina_core::{DuplicateGroup, Flag, MediaId, MediaItem, StackKind};

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::sidebar::AlbumSelection;

#[cfg(test)]
thread_local! {
    /// Makes the stack read fail on this test thread.
    pub(crate) static STACKS_LOAD_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

/// Which list the Duplicates destination shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuplicatesTab {
    /// Byte-identical files (the merge view).
    #[default]
    Exact,
    /// RAW+JPEG pairs and bursts.
    Similar,
}

/// A stack with its members' rows, in [`lumina_core::Stack::items`] order.
pub struct ReviewStack {
    pub kind: StackKind,
    pub items: Vec<MediaItem>,
}

/// The Similar shots list.
#[derive(Default)]
pub enum StacksList {
    /// A read is running (or the tab has not been opened since a reset).
    #[default]
    Loading,
    Loaded(Vec<ReviewStack>),
    /// The read failed; the raw error went to the log.
    Failed,
}

/// Tab, list and keyboard focus of the Duplicates destination.
#[derive(Default)]
pub struct StacksState {
    pub tab: DuplicatesTab,
    pub list: StacksList,
    /// The stack ↑/↓ move between and ↵ acts on.
    pub focused: usize,
    /// Bumped per read and per reset, so only the latest read lands.
    request: u64,
    pub scroll: UniformListScrollHandle,
    /// The Exact copies review's group selection.
    pub review: DuplicatesReview,
}

/// Which exact-copy group the review compares.
#[derive(Default)]
pub struct DuplicatesReview {
    /// Index into `duplicate_groups`. Read through [`Self::selected`]: the
    /// groups shrink after a merge, so the stored value may be past the end.
    selected: usize,
    pub scroll: UniformListScrollHandle,
    /// The copy each group keeps, by content hash, when the user chose one
    /// over the heuristic. Session only; read through [`Self::keeper`].
    pub keepers: HashMap<String, MediaId>,
}

impl DuplicatesReview {
    /// The selected group, clamped to the last of `len` groups; None when
    /// there are none.
    pub fn selected(&self, len: usize) -> Option<usize> {
        len.checked_sub(1).map(|last| self.selected.min(last))
    }

    /// The copy `group` keeps: the user's choice while it is still in the
    /// group, else the heuristic pick (the first item).
    pub fn keeper(&self, group: &DuplicateGroup) -> MediaId {
        self.keepers
            .get(&group.hash)
            .copied()
            .filter(|id| group.items.iter().any(|i| i.id == *id))
            .or_else(|| group.items.first().map(|i| i.id))
            .unwrap_or(MediaId(0))
    }

    /// Selects group `index` (clamped on read) and scrolls its row into view.
    pub fn select(&mut self, index: usize) {
        self.selected = index;
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
    }
}

impl StacksState {
    /// Back to Exact copies with no list; a read in flight is dropped.
    pub fn reset(&mut self) {
        self.review = DuplicatesReview::default();
        self.tab = DuplicatesTab::Exact;
        self.list = StacksList::Loading;
        self.focused = 0;
        self.request += 1;
    }

    fn stack(&self, index: usize) -> Option<&ReviewStack> {
        match &self.list {
            StacksList::Loaded(stacks) => stacks.get(index),
            _ => None,
        }
    }

    fn len(&self) -> usize {
        match &self.list {
            StacksList::Loaded(stacks) => stacks.len(),
            _ => 0,
        }
    }
}

impl LuminaApp {
    /// Shows `tab`. Opening Similar shots (again) reads the stacks afresh.
    /// The selection is cleared: Similar shots has none, and a selection
    /// left over from Exact copies must not be what ⌘E or I act on.
    pub fn set_duplicates_tab(&mut self, tab: DuplicatesTab, cx: &mut Context<Self>) {
        // From the viewer (⌘K): the viewer item is an Exact-copies selection
        // that Similar shots must not keep.
        if tab == DuplicatesTab::Similar && self.is_fullscreen_viewer {
            self.close_viewer(cx);
        }
        self.stacks.tab = tab;
        self.stacks.focused = 0;
        self.clear_selection(cx);
        if tab == DuplicatesTab::Similar {
            self.load_stacks(cx);
        }
        cx.notify();
    }

    /// The Exact copies review is on screen (its own header and actions).
    pub(crate) fn duplicates_review_shown(&self) -> bool {
        self.selection == AlbumSelection::Duplicates && self.stacks.tab == DuplicatesTab::Exact
    }

    /// Palette "Show Exact Copies" / "Show Similar Shots", from anywhere.
    pub(crate) fn show_duplicates_tab(&mut self, tab: DuplicatesTab, cx: &mut Context<Self>) {
        if self.compare_holds_view() {
            return;
        }
        if self.selection != AlbumSelection::Duplicates {
            self.select_album(AlbumSelection::Duplicates, cx);
        }
        self.set_duplicates_tab(tab, cx);
    }

    /// Reads the stacks on the background executor. A result lands only if
    /// no other read or reset (leaving the view, another library) came
    /// after it. The list is not re-read after flag writes: flags never
    /// change which items stack, and each member's marks are drawn from
    /// `item_flags`, which the write updates.
    pub(crate) fn load_stacks(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        self.stacks.request += 1;
        self.stacks.list = StacksList::Loading;
        let key = (self.library_generation, self.stacks.request);
        #[cfg(test)]
        let fail = STACKS_LOAD_FAILS.with(std::cell::Cell::get);
        #[cfg(not(test))]
        let fail = false;
        let task = cx.background_executor().spawn(async move {
            if fail {
                anyhow::bail!("database is locked");
            }
            let stacks = engine.find_stacks()?;
            let ids: Vec<MediaId> = stacks
                .iter()
                .flat_map(|s| s.items.iter().copied())
                .collect();
            let rows: std::collections::HashMap<MediaId, MediaItem> = engine
                .catalog()
                .get_items_by_ids(&ids)?
                .into_iter()
                .map(|item| (item.id, item))
                .collect();
            // `get`, not `remove`: an image is in its pair and in a burst.
            // A member that vanished between the two reads drops its stack:
            // a partial stack would make "the first" a different item.
            Ok(stacks
                .into_iter()
                .filter_map(|stack| {
                    let items: Option<Vec<MediaItem>> =
                        stack.items.iter().map(|id| rows.get(id).cloned()).collect();
                    Some(ReviewStack {
                        kind: stack.kind,
                        items: items?,
                    })
                })
                .collect())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                if (app.library_generation, app.stacks.request) != key {
                    return;
                }
                app.stacks.list = match result {
                    Ok(stacks) => StacksList::Loaded(stacks),
                    Err(err) => {
                        log::warn!("could not find similar shots: {err:?}");
                        StacksList::Failed
                    }
                };
                app.stacks.focused = 0;
                cx.notify();
            });
        })
        .detach();
    }

    /// Focuses stack `index` (clamped) and scrolls it into view.
    pub(crate) fn focus_stack(&mut self, index: usize, cx: &mut Context<Self>) {
        let last = self.stacks.len().saturating_sub(1);
        self.stacks.focused = index.min(last);
        self.stacks
            .scroll
            .scroll_to_item(self.stacks.focused, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// The stack's members, if every one is still in the library (not
    /// trashed, hidden, missing or gone). Otherwise the list may be stale
    /// (an item was trashed elsewhere): it is re-read, a toast says so, and
    /// nothing is flagged, since "the first" or the picks may have changed.
    fn live_members(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Vec<MediaId>> {
        let ids: Vec<MediaId> = self
            .stacks
            .stack(index)?
            .items
            .iter()
            .map(|i| i.id)
            .collect();
        let engine = self.grid_state.as_ref()?.engine.clone();
        // ponytail: one small sync read of this stack's rows on the UI
        // thread, right before the sync flag write that `set_flag_on` does
        // anyway; a few ids, one indexed query.
        let rows = match engine.catalog().get_items_by_ids(&ids) {
            Ok(rows) => rows,
            Err(err) => {
                log::warn!("could not read the stack: {err:?}");
                self.set_error(t("stacks.read_failed"), cx);
                return None;
            }
        };
        let live = rows.len() == ids.len()
            && rows
                .iter()
                .all(|r| !r.is_trashed && !r.is_hidden && !self.missing_items.contains(&r.id));
        if !live {
            self.show_toast(t("stacks.changed"), cx);
            self.load_stacks(cx);
            return None;
        }
        Some(ids)
    }

    /// "Keep picks, reject the rest": needs a pick (else a toast says so),
    /// then rejects every other member in one write and one undo entry.
    pub(crate) fn stack_keep_picks(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ids) = self.live_members(index, cx) else {
            return;
        };
        if !ids.iter().any(|id| self.item_flag(*id) == Flag::Pick) {
            self.show_toast(t("stacks.pick_first"), cx);
            return;
        }
        let rest = ids
            .into_iter()
            .filter(|id| self.item_flag(*id) != Flag::Pick);
        let rest = rest.collect();
        self.reject_in_stack(rest, cx);
    }

    /// "Reject all but the first": every member except `items[0]`.
    pub(crate) fn stack_reject_others(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(ids) = self.live_members(index, cx) else {
            return;
        };
        self.reject_in_stack(ids.into_iter().skip(1).collect(), cx);
    }

    /// Rejects `ids`, leaving the ones already rejected out:
    /// [`Self::set_flag_on`] toggles a set that all has the flag back to
    /// unflagged, which these actions must never do.
    fn reject_in_stack(&mut self, mut ids: Vec<MediaId>, cx: &mut Context<Self>) {
        ids.retain(|id| self.item_flag(*id) != Flag::Reject);
        if ids.is_empty() {
            self.show_warning_toast(t("stacks.nothing_to_reject"), cx);
            return;
        }
        self.set_flag_on(&ids, Flag::Reject, false, cx);
    }

    /// Whether Similar shots is what the window shows. While it is, the
    /// selection stays empty: `select_all`, `select_index` and
    /// `select_item_internal` refuse, so every selection action (menus,
    /// palette, ⌘E, ⇧⌘R…) has nothing to act on.
    pub(crate) fn similar_shots_shown(&self) -> bool {
        // No viewer term: Show Similar Shots closes the viewer, and nothing
        // opens it from here (no selection, Enter is taken, ⌥⌘S refuses).
        self.selection == AlbumSelection::Duplicates && self.stacks.tab == DuplicatesTab::Similar
    }

    /// Keys while Similar shots is shown: ↑/↓ focus, ↵ Keep picks (Try
    /// Again on the error state), ⇧↵ Reject all but the first. Esc falls
    /// through to the grid's Esc, as on Exact copies; every other key that
    /// gets here is absorbed, so a grid shortcut (P/X, F, ⌫, digits) never
    /// acts on a selection. Global shortcuts (⌘Z, ⌘K…) ran before this.
    pub(crate) fn handle_stacks_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.similar_shots_shown() {
            return false;
        }
        let m = &keystroke.modifiers;
        let plain = !(m.platform || m.control || m.alt || m.function);
        let focused = self.stacks.focused;
        let enter = matches!(keystroke.key.as_str(), "enter" | "return") && plain;
        match keystroke.key.as_str() {
            "escape" => return false,
            _ if enter && matches!(self.stacks.list, StacksList::Failed) => self.load_stacks(cx),
            "up" if plain && !m.shift => self.focus_stack(focused.saturating_sub(1), cx),
            "down" if plain && !m.shift => self.focus_stack(focused + 1, cx),
            _ if enter && !m.shift => self.stack_keep_picks(focused, cx),
            _ if enter => self.stack_reject_others(focused, cx),
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::DuplicatesReview;

    #[core::prelude::v1::test]
    fn review_selection_never_reads_past_the_groups() {
        let mut review = DuplicatesReview::default();
        assert_eq!(review.selected(0), None, "no groups, no selection");
        review.select(5);
        assert_eq!(review.selected(2), Some(1), "clamped to the last group");
        assert_eq!(review.selected(0), None, "groups merged away: none");
        assert_eq!(review.selected(9), Some(5), "in range: as selected");
    }
}
