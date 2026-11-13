//! Review Duplicates actions shared by the buttons, the keyboard path and
//! the ⌘K commands, and the review's key handler.

use gpui::*;
use lumina_core::{DuplicateGroup, MediaId};

use crate::app::LuminaApp;

impl LuminaApp {
    /// The shown group (index among the visible groups, and the group).
    fn shown_duplicate_group(&self) -> Option<(usize, &DuplicateGroup)> {
        let visible = self.stacks.review.visible(&self.duplicate_groups);
        let index = self.stacks.review.selected(&visible)?;
        Some((index, visible[index]))
    }

    /// Shows the visible group at `index` (clamped): ↑/↓, a row click and
    /// prev/next. The selection is cleared, so ⌫ and ⌘E never act on a
    /// copy of a group no longer shown.
    pub(crate) fn duplicates_select_group(&mut self, index: usize, cx: &mut Context<Self>) {
        let visible = self.stacks.review.visible(&self.duplicate_groups);
        let Some(last) = visible.len().checked_sub(1) else {
            return;
        };
        let index = index.min(last);
        let hash = visible[index].hash.clone();
        self.stacks.review.select(index, &hash);
        if self.selected_item.is_some() {
            self.clear_selection(cx);
        }
        cx.notify();
    }

    /// ↑/↓: the previous/next visible group.
    pub(crate) fn duplicates_step_group(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((index, _)) = self.shown_duplicate_group() else {
            return;
        };
        self.duplicates_select_group(index.saturating_add_signed(delta), cx);
    }

    /// 1–9: the shown group keeps its copy at `position`; past the last
    /// copy, nothing.
    pub(crate) fn duplicates_pick_keeper(&mut self, position: usize, cx: &mut Context<Self>) {
        let Some((_, group)) = self.shown_duplicate_group() else {
            return;
        };
        let Some(id) = group.items.get(position).map(|i| i.id) else {
            return;
        };
        let hash = group.hash.clone();
        self.stacks.review.keepers.insert(hash, id);
        cx.notify();
    }

    /// ←/→: the copy before/after the keeper becomes the keeper.
    pub(crate) fn duplicates_step_keeper(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some((_, group)) = self.shown_duplicate_group() else {
            return;
        };
        let keeper = self.stacks.review.keeper(group);
        let at = group.items.iter().position(|i| i.id == keeper).unwrap_or(0);
        let next = at.saturating_add_signed(delta);
        self.duplicates_pick_keeper(next, cx);
    }

    /// S / Skip group: hides the shown group; the next one is shown.
    pub(crate) fn duplicates_skip_shown(&mut self, cx: &mut Context<Self>) {
        let Some((index, group)) = self.shown_duplicate_group() else {
            return;
        };
        let hash = group.hash.clone();
        self.stacks.review.skip(index, &hash);
        cx.notify();
    }

    /// Skip all: hides every group until Duplicates is opened again.
    pub(crate) fn duplicates_skip_all(&mut self, cx: &mut Context<Self>) {
        let hashes: Vec<String> = self
            .duplicate_groups
            .iter()
            .map(|g| g.hash.clone())
            .collect();
        self.stacks.review.skipped.extend(hashes);
        cx.notify();
    }

    /// Auto-select best: every group back to the heuristic pick.
    pub(crate) fn duplicates_auto_best(&mut self, cx: &mut Context<Self>) {
        self.stacks.review.keepers.clear();
        cx.notify();
    }

    /// ↵ / Merge group: the shown group's other copies go to Recently
    /// Deleted (undoable); the next group is shown.
    pub(crate) fn duplicates_merge_shown(&mut self, cx: &mut Context<Self>) {
        let Some((index, group)) = self.shown_duplicate_group() else {
            return;
        };
        let keeper = self.stacks.review.keeper(group);
        let trash: Vec<MediaId> = group
            .items
            .iter()
            .map(|i| i.id)
            .filter(|id| *id != keeper)
            .collect();
        let hash = group.hash.clone();
        // Pinned here, so the group after it is shown next.
        self.stacks.review.select(index, &hash);
        self.merge_duplicate_group(keeper, &trash, cx);
    }

    /// ⌘↓: the selected copy in the viewer when it is in the shown group,
    /// else the keeper.
    fn duplicates_open_shown(&mut self, cx: &mut Context<Self>) {
        let Some((_, group)) = self.shown_duplicate_group() else {
            return;
        };
        let keeper = self.stacks.review.keeper(group);
        let selected = self.selected_item.as_ref().map(|i| i.id);
        let item = group
            .items
            .iter()
            .find(|i| Some(i.id) == selected)
            .or_else(|| group.items.iter().find(|i| i.id == keeper))
            .cloned();
        if let Some(item) = item {
            self.open_viewer(item, cx);
        }
    }

    /// The review's keys, before the grid's: ↑/↓ groups, ←/→ and 1–9 the
    /// keeper (never a rating), S skip, ↵ merge, ⌘↓ open; every other
    /// Enter, arrow, Home/End and PageUp/PageDown is absorbed. Runs after
    /// the modal and text-field shields; ⌘Z and the other global keys are
    /// handled before it. Other keys fall through.
    pub(crate) fn handle_duplicates_review_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.duplicates_review_shown() || self.is_fullscreen_viewer {
            return false;
        }
        let m = &keystroke.modifiers;
        let plain = !(m.platform || m.control || m.alt || m.function || m.shift);
        let cmd_only = (m.platform || m.control) && !m.alt && !m.shift;
        match keystroke.key.as_str() {
            "down" if cmd_only => self.duplicates_open_shown(cx),
            "up" if plain => self.duplicates_step_group(-1, cx),
            "down" if plain => self.duplicates_step_group(1, cx),
            "left" if plain => self.duplicates_step_keeper(-1, cx),
            "right" if plain => self.duplicates_step_keeper(1, cx),
            "s" if plain => self.duplicates_skip_shown(cx),
            "enter" | "return" if plain => self.duplicates_merge_shown(cx),
            // Digits pick the keeper; 0 and those past the copies do
            // nothing, and none reaches the culling keys' ratings.
            key if plain && key.len() == 1 && key.as_bytes()[0].is_ascii_digit() => {
                if let Some(position) = (key.as_bytes()[0] - b'0').checked_sub(1) {
                    self.duplicates_pick_keeper(position as usize, cx);
                }
            }
            // ⇧/⌥↵ and the grid's selection keys (⇧+arrows, Home/End,
            // PageUp/PageDown) would open or select hidden grid items.
            "enter" | "return" | "up" | "down" | "left" | "right" | "home" | "end" | "pageup"
            | "pagedown" => {}
            _ => return false,
        }
        true
    }
}
