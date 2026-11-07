//! Keyboard-first culling: pick/reject flags and the plain-key flag and
//! rating shortcuts shared by the grid and the viewer.

use gpui::*;
use lumina_core::{Flag, MediaId};

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;

#[cfg(test)]
thread_local! {
    /// Makes the flag write fail without writing on this test thread: the
    /// UI crate has no SQL handle to break.
    pub(crate) static FLAG_WRITE_FAILS: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

impl LuminaApp {
    /// The item's flag from the cached map (never the catalog).
    pub(crate) fn item_flag(&self, id: MediaId) -> Flag {
        self.item_flags.get(&id).copied().unwrap_or(Flag::Unflagged)
    }

    /// Gives the selection `flag`, or clears it when every selected item
    /// already has it: one write and one undo entry per batch. Flags are not
    /// written to sidecars. Returns `false` when nothing is selected or the
    /// write failed (the banner says why and nothing changes).
    pub fn set_flag_selected(&mut self, flag: Flag, cx: &mut Context<Self>) -> bool {
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return false;
        }
        let new = if ids.iter().all(|id| self.item_flag(*id) == flag) {
            Flag::Unflagged
        } else {
            flag
        };
        let changes: Vec<(MediaId, Flag, Flag)> = ids
            .iter()
            .map(|id| (*id, self.item_flag(*id), new))
            .filter(|(_, before, after)| before != after)
            .collect();
        if changes.is_empty() {
            cx.notify();
            return true;
        }
        let writes: Vec<(MediaId, Flag)> = changes.iter().map(|&(id, _, a)| (id, a)).collect();
        let held = self.hold_selection(&ids, writes.len());
        if !self.flag_write(&writes, cx) {
            return false;
        }
        for &(id, value) in &writes {
            if value == Flag::Unflagged {
                self.item_flags.remove(&id);
            } else {
                self.item_flags.insert(id, value);
            }
        }
        let count = writes.len();
        self.push_undo(UndoAction::Flag { changes });
        // Show Rejected / Show Picks drop the item: keep the selection by id.
        self.reselect_held(held, cx);
        // A saved search can filter on is:pick / is:reject.
        self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
        self.show_undo_toast(
            match new {
                Flag::Pick => format!("Picked {count} item(s)"),
                Flag::Reject => format!("Rejected {count} item(s)"),
                Flag::Unflagged => "Flag removed".to_string(),
            },
            cx,
        );
        true
    }

    /// The one flag write: the test hook makes it fail without writing.
    fn flag_write(&mut self, writes: &[(MediaId, Flag)], cx: &mut Context<Self>) -> bool {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return false;
        };
        #[cfg(test)]
        let result = if FLAG_WRITE_FAILS.with(std::cell::Cell::get) {
            Err(anyhow::anyhow!("database is locked"))
        } else {
            engine.set_flags(writes)
        };
        #[cfg(not(test))]
        let result = engine.set_flags(writes);
        match result {
            Ok(()) => true,
            Err(err) => {
                log::warn!("could not save flag: {err:?}");
                self.set_error(format!("Could not save flag: {err:#}"), cx);
                false
            }
        }
    }

    /// P pick, X reject, U unflag and 0-5 rate, with no modifier (⌥P is
    /// PiP, ⌥⌘U Import Video from URL). In the viewer a key on one item
    /// moves to the next one (it stays on the last), whenever nothing
    /// failed, even if the item already had that value (U on an unflagged
    /// item still advances, as in Lightroom). The next item is found by id,
    /// since the write may drop the shown item from a filtered view. In the
    /// grid the selection never moves. Returns `true` when the key was one
    /// of these.
    pub(crate) fn handle_culling_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        let m = &keystroke.modifiers;
        if m.platform || m.control || m.alt || m.shift || m.function {
            return false;
        }
        // Read before the write: afterwards positions may name other items.
        // `(i, next id)`: the shown row and the item after it.
        let next = if self.is_fullscreen_viewer && self.get_selected_ids().len() == 1 {
            self.selected_index
                .and_then(|index| self.navigation_item(index + 1).map(|item| (index, item.id)))
        } else {
            None
        };
        let done = match keystroke.key.as_str() {
            "p" => self.set_flag_selected(Flag::Pick, cx),
            "x" => self.set_flag_selected(Flag::Reject, cx),
            "u" => self.set_flag_selected(Flag::Unflagged, cx),
            digit @ ("0" | "1" | "2" | "3" | "4" | "5") => {
                let rating = digit.parse().unwrap_or(0);
                self.set_rating_selected(rating, cx)
            }
            _ => return false,
        };
        let target = next
            .filter(|_| done)
            // One item changed, so the next one (at i + 1 before the write)
            // moved by at most one row. The pre-write i, not the shown
            // item's new row: sorted by Rating the rated item can sink
            // past it.
            .and_then(|(index, id)| self.positions_in(&[id], index, index + 2).get(&id).copied());
        if let Some(index) = target {
            self.viewer_state.reset();
            self.select_index(index, cx);
        }
        true
    }
}
