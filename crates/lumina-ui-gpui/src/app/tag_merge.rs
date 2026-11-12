//! Merge into… for one keyword (detail ORGANISE row, row `…` menu, ⌘K
//! "Merge Tag Into…"): a target picker filtered by typing, ↑↓ and ↵.

use gpui::*;
use lumina_core::catalog::keywords::subtree_suffix;
use lumina_core::TagChange;

use crate::app::state::TagMergePicker;
use crate::app::LuminaApp;
use crate::i18n::t;

impl LuminaApp {
    /// Opens the target picker for the command's keyword.
    pub fn start_tag_merge(&mut self, cx: &mut Context<Self>) {
        if !self.focus_command_keyword(cx) {
            return;
        }
        self.tag_manager.editing = None;
        self.tag_manager.editing_buffer.clear();
        self.tag_manager.moving = false;
        self.tag_manager.alias_editing = false;
        self.tag_manager.merge = Some(TagMergePicker::default());
        cx.notify();
    }

    pub fn cancel_tag_merge(&mut self, cx: &mut Context<Self>) {
        self.tag_manager.merge = None;
        cx.notify();
    }

    /// Keywords the focused one can merge into, A–Z by segment, matching
    /// the filter: never itself or a keyword below it (the catalog refuses
    /// those).
    // ponytail: O(keywords) per frame, in memory, like the list filter.
    pub(crate) fn tag_merge_options(&self) -> Vec<String> {
        let (Some(source), Some(picker)) = (
            self.tag_manager.focused.as_deref(),
            self.tag_manager.merge.as_ref(),
        ) else {
            return Vec::new();
        };
        let filter = picker.filter.trim().to_lowercase();
        let mut options: Vec<String> = self
            .library_tag_counts
            .iter()
            .map(|(tag, _)| tag)
            .filter(|tag| subtree_suffix(tag, source).is_none())
            .filter(|tag| tag.contains(&filter))
            .cloned()
            .collect();
        options.sort_by(|a, b| a.split('/').cmp(b.split('/')));
        options
    }

    /// The highlighted target, held inside the current options.
    pub(crate) fn tag_merge_highlight(&self) -> usize {
        let last = self.tag_merge_options().len().saturating_sub(1);
        self.tag_manager
            .merge
            .as_ref()
            .map_or(0, |picker| picker.highlight.min(last))
    }

    /// ↑/↓ in the picker.
    pub(crate) fn step_tag_merge(&mut self, delta: isize, cx: &mut Context<Self>) {
        let last = self.tag_merge_options().len().saturating_sub(1);
        let at = self.tag_merge_highlight();
        if let Some(picker) = self.tag_manager.merge.as_mut() {
            picker.highlight = at.saturating_add_signed(delta).min(last);
        }
        cx.notify();
    }

    /// ↵: merges the focused keyword into the highlighted target.
    pub(crate) fn commit_tag_merge(&mut self, cx: &mut Context<Self>) {
        let target = self
            .tag_merge_options()
            .into_iter()
            .nth(self.tag_merge_highlight());
        if let (Some(source), Some(target)) = (self.tag_manager.focused.clone(), target) {
            self.merge_tag_into(source, target, cx);
        }
    }

    /// Merges `source` (and its subtree) into `target` as one undo step; the
    /// catalog keeps `source` as an alias of `target`. A failure goes to the
    /// error banner and leaves the picker open; nothing changed before the
    /// write, so there is no state to revert.
    pub fn merge_tag_into(&mut self, source: String, target: String, cx: &mut Context<Self>) {
        let result = match self.grid_state.as_ref() {
            Some(state) => state.engine.merge_tags(&source, &target),
            None => Ok(Default::default()),
        };
        let Some(change) = self.apply_tag_mutation("Could not merge tags", result, cx) else {
            cx.notify();
            return;
        };
        self.tag_manager.merge = None;
        self.sync_metadata_sidecars(&change.affected_ids(), cx);
        let view_before = self.tag_view_refs();
        self.retarget_tag_refs(&source, Some(&target), cx);
        let moves = vec![(source.clone(), target.clone())];
        let recorded = self.push_tag_rows_undo("tag merge", change, view_before, false, moves);
        self.refresh_keyword_aliases();
        self.refresh_selected_tags();
        self.refresh_album_counts();
        self.focus_tag_manager_tag(target.clone(), cx);
        let message = t("tags.merged_into")
            .replace("{source}", &source)
            .replace("{target}", &target);
        self.tag_rows_toast(recorded, message, cx);
        cx.notify();
    }

    /// The most-used checked tag (ties: A–Z): Merge folds the others into it.
    pub(crate) fn tag_merge_target(&self) -> Option<String> {
        self.library_tag_counts
            .iter()
            .filter(|(tag, _)| self.tag_manager.selected.contains(tag))
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(tag, _)| tag.clone())
    }

    /// Merges every checked tag into [`Self::tag_merge_target`] (v3 Merge).
    pub fn merge_checked_tags(&mut self, cx: &mut Context<Self>) {
        if self.tag_manager.selected.len() < 2 {
            return;
        }
        let Some(target) = self.tag_merge_target() else {
            return;
        };
        let mut sources: Vec<String> = self
            .tag_manager
            .selected
            .iter()
            .filter(|tag| **tag != target)
            .cloned()
            .collect();
        sources.sort();
        let mut merged = 0;
        // Every merge folds into one change, so one undo reverses them all.
        let mut combined = TagChange::default();
        let view_before = self.tag_view_refs();
        for source in &sources {
            let result = match self.grid_state.as_ref() {
                Some(state) => state.engine.merge_tags(source, &target),
                None => Ok(Default::default()),
            };
            // Stop at the first failure; the merges before it are committed
            // and the counts and sidecars below reflect them.
            let Some(change) = self.apply_tag_mutation("Could not merge tags", result, cx) else {
                break;
            };
            combined.then(change);
            merged += 1;
            self.retarget_tag_refs(source, Some(&target), cx);
        }
        if merged == sources.len() {
            self.tag_manager.selected.clear();
        }
        self.sync_metadata_sidecars(&combined.affected_ids(), cx);
        // Each merge keeps its source name as an alias of the target.
        self.refresh_keyword_aliases();
        self.refresh_selected_tags();
        self.refresh_album_counts();
        self.focus_tag_manager_tag(target.clone(), cx);
        // On a failure part way the merges before it are committed: they
        // still undo as one step.
        let moves = sources[..merged]
            .iter()
            .map(|source| (source.clone(), target.clone()))
            .collect();
        let recorded = self.push_tag_rows_undo("tag merge", combined, view_before, false, moves);
        if merged > 0 {
            let message = format!("Merged {merged} tag(s) into \"{target}\"");
            self.tag_rows_toast(recorded, message, cx);
        }
        cx.notify();
    }
}
