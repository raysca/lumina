//! Back/Forward browsing trail: destinations and the bounded cursor history.

use lumina_core::MediaId;
use std::collections::VecDeque;

use crate::views::sidebar::AlbumSelection;
/// Longest browsing trail kept for Back. Older destinations fall off
/// the front so the stack stays bounded no matter how long a session runs.
pub(crate) const NAVIGATION_HISTORY_LIMIT: usize = 50;

/// A restorable browsing destination.
///
/// Only durable navigation state belongs here: transient overlays (menus,
/// toasts, inline editors) must not reset the user's place when they go back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NavigationSnapshot {
    pub selection: AlbumSelection,
    pub search_query: String,
    pub active_year: Option<i32>,
    pub selected_media_id: Option<MediaId>,
    pub selected_index: Option<usize>,
    pub selection_anchor: Option<usize>,
    pub viewer_open: bool,
}

impl Default for NavigationSnapshot {
    fn default() -> Self {
        Self {
            selection: AlbumSelection::AllLibrary,
            search_query: String::new(),
            active_year: None,
            selected_media_id: None,
            selected_index: None,
            selection_anchor: None,
            viewer_open: false,
        }
    }
}

impl NavigationSnapshot {
    /// Whether two snapshots point at the same browsing destination.
    ///
    /// Selection is deliberately excluded: arrow keys move the selection
    /// without navigating and must not flood the trail with entries.
    fn same_destination(&self, other: &Self) -> bool {
        self.selection == other.selection
            && self.search_query == other.search_query
            && self.active_year == other.active_year
            && self.viewer_open == other.viewer_open
    }

    fn remember_selection(&mut self, other: &Self) {
        self.selected_media_id = other.selected_media_id;
        self.selected_index = other.selected_index;
        self.selection_anchor = other.selection_anchor;
    }
}

/// Bounded Back/Forward trail of browsing destinations.
#[derive(Debug)]
pub(crate) struct NavigationHistory {
    entries: VecDeque<NavigationSnapshot>,
    cursor: usize,
    restoring: bool,
}

impl NavigationHistory {
    pub(crate) fn new(initial: NavigationSnapshot) -> Self {
        let mut entries = VecDeque::with_capacity(NAVIGATION_HISTORY_LIMIT);
        entries.push_back(initial);
        Self {
            entries,
            cursor: 0,
            restoring: false,
        }
    }

    pub(crate) fn is_restoring(&self) -> bool {
        self.restoring
    }

    pub(crate) fn begin_restore(&mut self) {
        self.restoring = true;
    }

    pub(crate) fn end_restore(&mut self) {
        self.restoring = false;
    }

    /// True when the live state left the recorded cursor without a snapshot,
    /// for example a search typed straight into the field or a viewer opened
    /// from the grid.
    pub(crate) fn has_pending_destination(&self, snapshot: &NavigationSnapshot) -> bool {
        match self.entries.get(self.cursor) {
            Some(current) => !current.same_destination(snapshot),
            None => true,
        }
    }

    /// Materializes the live destination if the app moved without an explicit
    /// record call. Called before Back so the place being left stays reachable.
    pub(crate) fn sync_pending(&mut self, snapshot: NavigationSnapshot) {
        if self.restoring {
            return;
        }
        if !self.has_pending_destination(&snapshot) {
            if let Some(current) = self.entries.get_mut(self.cursor) {
                current.remember_selection(&snapshot);
            }
            return;
        }
        if self.entries.is_empty() {
            self.entries.push_back(snapshot);
            self.cursor = 0;
            return;
        }
        // Opening the viewer happens over the grid, so the state underneath is
        // the same destination with the viewer closed.
        if snapshot.viewer_open && !self.entries[self.cursor].viewer_open {
            let mut grid = snapshot.clone();
            grid.viewer_open = false;
            self.entries[self.cursor] = grid;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push_back(snapshot);
        self.trim_front();
        self.cursor = self.entries.len().saturating_sub(1);
    }

    /// Appends a destination change. `departing` is the state before the
    /// change, so going Back restores the selection the user actually left.
    pub(crate) fn record(&mut self, departing: NavigationSnapshot, arriving: NavigationSnapshot) {
        if self.restoring || departing.same_destination(&arriving) {
            return;
        }
        if self.entries.is_empty() {
            self.entries.push_back(departing);
            self.cursor = 0;
        } else if self.entries[self.cursor].same_destination(&departing) {
            self.entries[self.cursor] = departing;
        } else {
            // The live state drifted before this explicit change; keep it as a
            // destination rather than silently dropping a visited place.
            self.entries.truncate(self.cursor + 1);
            self.entries.push_back(departing);
            self.trim_front();
            self.cursor = self.entries.len().saturating_sub(1);
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push_back(arriving);
        self.trim_front();
        self.cursor = self.entries.len().saturating_sub(1);
    }

    pub(crate) fn go_back(&mut self) -> Option<NavigationSnapshot> {
        if self.restoring || self.cursor == 0 || self.entries.is_empty() {
            return None;
        }
        self.cursor -= 1;
        self.entries.get(self.cursor).cloned()
    }

    /// Redoes a Back: steps to the newer entry `record` kept after the cursor.
    pub(crate) fn go_forward(&mut self) -> Option<NavigationSnapshot> {
        if self.restoring || !self.can_go_forward() {
            return None;
        }
        self.cursor += 1;
        self.entries.get(self.cursor).cloned()
    }

    pub(crate) fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub(crate) fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Keeps the record in sync when a saved album/tag/media had to fall back
    /// to the nearest valid view.
    pub(crate) fn replace_current(&mut self, snapshot: NavigationSnapshot) {
        if let Some(current) = self.entries.get_mut(self.cursor) {
            *current = snapshot;
        }
    }

    /// The entry under the cursor, for tests that check the trail itself.
    #[cfg(test)]
    pub(crate) fn current(&self) -> Option<&NavigationSnapshot> {
        self.entries.get(self.cursor)
    }

    fn trim_front(&mut self) {
        while self.entries.len() > NAVIGATION_HISTORY_LIMIT {
            self.entries.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(tag: &str) -> NavigationSnapshot {
        NavigationSnapshot {
            selection: AlbumSelection::Tag(tag.to_string()),
            ..NavigationSnapshot::default()
        }
    }

    /// Records A -> B -> ... the way `record_navigation_change` does.
    fn trail(tags: &[&str]) -> NavigationHistory {
        let mut history = NavigationHistory::new(at(tags[0]));
        for pair in tags.windows(2) {
            history.record(at(pair[0]), at(pair[1]));
        }
        history
    }

    #[test]
    fn forward_after_back_returns_to_the_newer_entry() {
        let mut history = trail(&["a", "b"]);
        assert_eq!(history.go_back(), Some(at("a")));
        assert!(history.can_go_forward(), "Back leaves B ahead");
        assert_eq!(history.go_forward(), Some(at("b")), "Forward redoes Back");
        assert!(!history.can_go_forward());
        assert!(history.can_go_back());
    }

    #[test]
    fn new_destination_truncates_forward() {
        let mut history = trail(&["a", "b", "c"]);
        history.go_back();
        history.go_back();
        history.record(at("a"), at("d"));
        assert!(
            !history.can_go_forward(),
            "a new destination after Back must drop the Forward entries"
        );
        assert_eq!(history.go_back(), Some(at("a")));
        assert_eq!(history.go_forward(), Some(at("d")));
    }

    #[test]
    fn forward_at_end_is_none() {
        let mut history = trail(&["a", "b"]);
        assert!(!history.can_go_forward());
        assert_eq!(history.go_forward(), None, "nothing ahead of the newest");
        let mut fresh = NavigationHistory::new(at("a"));
        assert!(!fresh.can_go_back() && !fresh.can_go_forward());
        assert_eq!(fresh.go_forward(), None);
    }

    #[test]
    fn forward_is_none_while_restoring() {
        let mut history = trail(&["a", "b"]);
        history.go_back();
        history.begin_restore();
        assert_eq!(history.go_forward(), None, "no Forward mid-restore");
        history.end_restore();
        assert_eq!(history.go_forward(), Some(at("b")));
    }

    #[test]
    fn limit_trims_front_and_keeps_forward_reachable() {
        let names: Vec<String> = (0..=55).map(|i| format!("t{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut history = trail(&refs);
        assert_eq!(history.entries.len(), NAVIGATION_HISTORY_LIMIT);
        for _ in 0..3 {
            assert!(history.go_back().is_some());
        }
        let mut last = None;
        for _ in 0..3 {
            last = history.go_forward();
        }
        assert_eq!(last, Some(at("t55")), "Forward reaches the newest entry");
        assert!(!history.can_go_forward());
    }

    #[test]
    fn pending_destination_drops_forward() {
        let mut history = trail(&["a", "b"]);
        history.go_back();
        history.sync_pending(at("c"));
        assert!(
            !history.can_go_forward(),
            "materialising a drifted live state must drop the Forward entries"
        );
        assert_eq!(history.go_back(), Some(at("a")));
    }
}
