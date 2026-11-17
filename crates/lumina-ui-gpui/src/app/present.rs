//! Compare / survey mode (plan 5 C1, Decision 6): 2-9 selected items side
//! by side, 2×2 or 3×3, with one shared zoom/pan and keys that act on the
//! focused pane by id, never on the grid selection behind it.
//!
//! Memory stays bounded: two items decode at the viewer's 2048 px, more at
//! 1024 px, so a 3×3 holds nine 1024² textures, not nine 2048² ones. Every
//! texture compare owns is evicted from the GPU atlas with
//! `cx.drop_image(image, Some(window))` (through `retired_images`, drained by
//! the next render) when its pane is dropped or compare closes; dropping the
//! `Arc` alone would leave it in the atlas.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::*;
use lumina_core::{Flag, MediaId, MediaItem};

use crate::app::selection_by_id::HeldSelection;
use crate::app::undo::UndoAction;
use crate::app::viewer_actions::VIEWER_IMAGE_SIZE;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::views::viewer_state::{ViewerState, ViewerZoomPreset};

/// The most items compare shows (3×3).
pub(crate) const COMPARE_MAX: usize = 9;
/// Decode size for three or more panes.
pub(crate) const SURVEY_IMAGE_SIZE: u32 = 1024;

/// Tells one compare session's decodes from another's.
static GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
thread_local! {
    /// Decode size of each compare request sent from this test thread.
    pub(crate) static COMPARE_DECODE_SIZES: std::cell::RefCell<Vec<u32>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// An open compare / survey view.
pub(crate) struct CompareState {
    /// The compared rows, in grid order at open time.
    pub(crate) items: Vec<MediaItem>,
    /// Index into `items` of the focused pane.
    pub(crate) focus: usize,
    /// Decoded images; compare is their only owner.
    pub(crate) textures: HashMap<MediaId, Arc<RenderImage>>,
    /// Items whose decode gave up ("No preview").
    pub(crate) failed: HashSet<MediaId>,
    /// One zoom/pan for every pane, so they move together. It never has a
    /// canvas: presets are multiples of Fit and Z toggles Fit / 2x.
    pub(crate) view: ViewerState,
    pub(crate) generation: u64,
    /// The compare set's grid selection by id, taken at open and widened by
    /// every write since (compare's own and the viewer's opened from it),
    /// so ↵ and closing find the chosen items by id, never by stale position.
    held: Option<HeldSelection>,
    /// `UndoState::moves` when `held` was last widened.
    moves_seen: usize,
}

impl CompareState {
    pub(crate) fn focused_id(&self) -> Option<MediaId> {
        self.items.get(self.focus).map(|item| item.id)
    }
}

/// Columns for `count` panes: 2 side by side, 3-4 as 2×2, 5-9 as 3×3.
pub(crate) fn columns(count: usize) -> usize {
    match count {
        0..=1 => 1,
        2..=4 => 2,
        _ => 3,
    }
}

fn decode_size(count: usize) -> u32 {
    if count <= 2 {
        VIEWER_IMAGE_SIZE
    } else {
        SURVEY_IMAGE_SIZE
    }
}

impl LuminaApp {
    /// C in the grid, View > Compare, ⌘K Compare: the 2-9 selected items.
    /// Fewer or more shows a toast and opens nothing.
    pub fn open_compare(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.grid_state.as_ref().map(|s| s.engine.clone()) else {
            return;
        };
        // Under a dialog or the palette (the menu bar stays reachable), its
        // keys would act on compare.
        if self.compare.is_some() || self.is_fullscreen_viewer || self.import_blocked_by_modal() {
            return;
        }
        // Counted before any row is read: ⌘A on 100k items must not fetch.
        let count = if self.selected_indices.is_empty() {
            usize::from(self.selected_item.is_some())
        } else {
            self.selected_indices.len()
        };
        if count > COMPARE_MAX {
            self.show_warning_toast(t("compare.too_many"), cx);
            return;
        }
        let items = if count >= 2 {
            self.get_selected_items()
        } else {
            Vec::new()
        };
        if items.len() < 2 {
            self.show_warning_toast(t("compare.too_few"), cx);
            return;
        }
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
        let size = decode_size(items.len());
        for item in &items {
            #[cfg(test)]
            COMPARE_DECODE_SIZES.with(|sizes| sizes.borrow_mut().push(size));
            request_frame(&engine, generation, item.id, size, cx);
        }
        self.search_focused = false;
        let held = Some(self.hold_selection(&self.get_selected_ids(), 0));
        self.compare = Some(CompareState {
            items,
            focus: 0,
            textures: HashMap::new(),
            failed: HashSet::new(),
            view: ViewerState::default(),
            generation,
            held,
            moves_seen: self.undo.moves,
        });
        cx.notify();
    }

    /// Compare is open, shown or under the viewer opened from it. What would
    /// play, move or trash the view behind it (Back/Forward, Slideshow, Show
    /// Map, Similar shots, Delete Rejected…) is refused, and the menu bar and
    /// the palette leave it out, so compare never sits on another view.
    pub(crate) fn compare_holds_view(&self) -> bool {
        self.compare.is_some()
    }

    /// Closes compare and evicts every texture it holds.
    pub(crate) fn close_compare(&mut self, cx: &mut Context<Self>) {
        self.sync_compare_moves();
        let Some(state) = self.compare.take() else {
            return;
        };
        // Compare is the only owner, so no `retire_image` refcount check.
        self.retired_images.extend(state.textures.into_values());
        // Back on the chosen items still in view, by id (cleared when none
        // is).
        if let Some(held) = state.held {
            self.reselect_held(held, cx);
        }
        cx.notify();
    }

    /// A decoded image for pane `id`. One for a closed compare, a dropped
    /// pane or an earlier session is evicted at once.
    pub(crate) fn land_compare_image(
        &mut self,
        generation: u64,
        id: MediaId,
        image: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        let state = self
            .compare
            .as_mut()
            .filter(|s| s.generation == generation && s.items.iter().any(|i| i.id == id));
        let Some(state) = state else {
            self.retired_images.push(image);
            cx.notify();
            return;
        };
        if let Some(old) = state.textures.insert(id, image) {
            self.retired_images.push(old);
        }
        cx.notify();
    }

    fn compare_decode_failed(&mut self, generation: u64, id: MediaId, cx: &mut Context<Self>) {
        if let Some(state) = self.compare.as_mut().filter(|s| s.generation == generation) {
            state.failed.insert(id);
            cx.notify();
        }
    }

    /// ←/→ (clamped) and Tab/⇧Tab (wrapping) move the focused pane.
    fn move_compare_focus(&mut self, delta: isize, wrap: bool, cx: &mut Context<Self>) {
        let Some(state) = self.compare.as_mut() else {
            return;
        };
        let len = state.items.len() as isize;
        let next = state.focus as isize + delta;
        state.focus = if wrap {
            next.rem_euclid(len.max(1))
        } else {
            next.clamp(0, (len - 1).max(0))
        } as usize;
        cx.notify();
    }

    /// Clicking a pane focuses it.
    pub(crate) fn focus_compare_pane(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(state) = self.compare.as_mut().filter(|s| index < s.items.len()) {
            state.focus = index;
            cx.notify();
        }
    }

    /// ⌫: takes the focused pane out of compare. Media is never touched.
    /// Dropping the last pane closes compare.
    fn drop_compare_pane(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.compare.as_mut() else {
            return;
        };
        if state.focus >= state.items.len() {
            return;
        }
        let item = state.items.remove(state.focus);
        state.failed.remove(&item.id);
        let texture = state.textures.remove(&item.id);
        state.focus = state.focus.min(state.items.len().saturating_sub(1));
        let empty = state.items.is_empty();
        self.retired_images.extend(texture);
        if empty {
            self.close_compare(cx);
        }
        cx.notify();
    }

    /// Shared zoom/pan of every pane (keys, scroll, drag).
    pub(crate) fn compare_view(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut ViewerState),
    ) {
        if let Some(state) = self.compare.as_mut() {
            f(&mut state.view);
            cx.notify();
        }
    }

    /// Compare's keys. Returns `false` when compare is not showing (closed,
    /// or the viewer opened from it is on top); otherwise every key stops
    /// here, so ⌘A, ⌫'s delete paths and the Delete dialog never act on the
    /// grid selection hidden behind it.
    pub(crate) fn handle_compare_key(
        &mut self,
        keystroke: &Keystroke,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.compare.is_none() || self.is_fullscreen_viewer {
            return false;
        }
        let m = &keystroke.modifiers;
        let key = keystroke.key.as_str();
        let plain = !m.platform && !m.control && !m.alt && !m.function;
        let exactly_cmd = m.secondary(cx) && !m.control && !m.shift && !m.alt;
        match key {
            "escape" if self.error_message.is_some() => self.dismiss_error(cx),
            "escape" => self.close_compare(cx),
            "left" | "right" if plain && !m.shift => {
                self.move_compare_focus(if key == "left" { -1 } else { 1 }, false, cx)
            }
            "tab" if plain => self.move_compare_focus(if m.shift { -1 } else { 1 }, true, cx),
            "backspace" | "delete" if plain && !m.shift => self.drop_compare_pane(cx),
            "enter" | "return" if plain => self.open_compare_item_in_viewer(cx),
            k if (m.secondary(cx) || m.control) && k.eq_ignore_ascii_case("z") => {
                self.compare_history(m.shift, cx)
            }
            "=" | "+" if plain => self.compare_view(cx, ViewerState::zoom_in),
            "-" | "_" if plain => self.compare_view(cx, ViewerState::zoom_out),
            k if plain && !m.shift && k.eq_ignore_ascii_case("z") => {
                self.compare_view(cx, ViewerState::toggle_zoom)
            }
            "0" | "1" | "2" | "3" | "4" if exactly_cmd => {
                let preset = match key {
                    "0" => ViewerZoomPreset::Fit,
                    "1" => ViewerZoomPreset::Actual,
                    "2" => ViewerZoomPreset::TwoX,
                    "3" => ViewerZoomPreset::Fill,
                    _ => ViewerZoomPreset::FourX,
                };
                self.compare_view(cx, |view| view.apply_preset(preset));
            }
            _ if plain && !m.shift => self.compare_mark_key(key, cx),
            _ => {}
        }
        true
    }

    /// Widens the held set by every row move the history saw since the
    /// last sync: compare's own writes, its ⌘Z / ⇧⌘Z, and every write,
    /// undo or redo in the viewer opened from it (`UndoState::moves` only
    /// grows, so an undo followed by a new step, or a full stack, still
    /// counts).
    /// ponytail: writes with no history entry are not seen: permanent
    /// delete or Empty Trash from the viewer's Delete dialog, duplicate
    /// merge, rescan, watcher or import rows, and a sort, search or filter
    /// change made from the viewer. Each ends in a safe
    /// clear (the held id is not found, nothing unchosen is selected); a
    /// core position-of-id query is the upgrade.
    fn sync_compare_moves(&mut self) {
        let now = self.undo.moves;
        if let Some(state) = self.compare.as_mut() {
            if let Some(held) = state.held.as_mut() {
                held.widen(now.saturating_sub(state.moves_seen));
            }
            state.moves_seen = now;
        }
    }

    /// ↵: the viewer on the focused item, with the grid selection made that
    /// one item at its real position (by id), so the viewer's keys, ←/→ and
    /// auto-advance act on it and its neighbours, never on the compare set.
    fn open_compare_item_in_viewer(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.live_focused_item(cx) else {
            return;
        };
        self.sync_compare_moves();
        let count = self.navigation_count();
        let window = self
            .compare
            .as_ref()
            .and_then(|s| s.held.as_ref())
            .and_then(|held| held.window(count));
        let position = window
            .and_then(|(lo, hi)| self.positions_in(&[item.id], lo, hi).get(&item.id).copied());
        match position {
            Some(index) => self.select_index(index, cx),
            // Not in the view any more (a flag in Show Picks): the viewer
            // shows it alone, with nothing selected behind it.
            None => self.clear_selection(cx),
        }
        self.open_viewer(item, cx);
    }

    /// ⌘Z / ⇧⌘Z: only a flag or rating step (what compare itself writes)
    /// runs here; older history (a trash, a tag) waits until compare closes.
    fn compare_history(&mut self, redo: bool, cx: &mut Context<Self>) {
        let stack = if redo {
            &self.undo.redo_stack
        } else {
            &self.undo.undo_stack
        };
        match stack.last() {
            Some(UndoAction::Flag { .. } | UndoAction::Rating { .. }) => {}
            Some(_) => {
                self.show_warning_toast(t("compare.history_outside"), cx);
                return;
            }
            None => return,
        }
        if redo {
            self.redo_last(cx);
        } else {
            self.undo_last(cx);
        }
    }

    /// The focused pane's row, read fresh: a pane whose item was trashed
    /// (through the viewer) or removed (watcher) is dropped instead, so no
    /// key writes to it.
    /// ponytail: one primary-key catalog read on the UI thread per mark key
    /// or ↵ (never per frame). The upgrade is pruning panes from the trash
    /// and watcher events, so keys need no read at all.
    fn live_focused_item(&mut self, cx: &mut Context<Self>) -> Option<MediaItem> {
        let id = self.compare.as_ref().and_then(CompareState::focused_id)?;
        let engine = self.grid_state.as_ref().map(|s| s.engine.clone())?;
        match engine.catalog().get_item_by_id(id) {
            Ok(Some(item)) if !item.is_trashed => Some(item),
            Ok(_) => {
                self.drop_compare_pane(cx);
                self.show_warning_toast(t("compare.pane_gone"), cx);
                None
            }
            Err(err) => {
                log::warn!("could not read compared item {id:?}: {err:?}");
                self.set_error(format!("Could not read the item: {err:#}"), cx);
                None
            }
        }
    }

    /// P/X/U and 0-5 on the focused pane by id, through the culling
    /// funnels (one write, undo, banner on failure).
    fn compare_mark_key(&mut self, key: &str, cx: &mut Context<Self>) {
        if !matches!(key, "p" | "x" | "u" | "0" | "1" | "2" | "3" | "4" | "5") {
            return;
        }
        let Some(id) = self.live_focused_item(cx).map(|item| item.id) else {
            return;
        };
        match key {
            "p" => self.set_flag_on(&[id], Flag::Pick, false, cx),
            "x" => self.set_flag_on(&[id], Flag::Reject, false, cx),
            "u" => self.set_flag_on(&[id], Flag::Unflagged, false, cx),
            digit @ ("0" | "1" | "2" | "3" | "4" | "5") => {
                self.set_rating_on(&[id], digit.parse().unwrap_or(0), false, cx)
            }
            _ => return,
        };
    }
}

/// How many rows a history entry can move: its changed items.
pub(crate) fn history_moves(action: &UndoAction) -> usize {
    match action {
        UndoAction::Flag { changes } => changes.len(),
        UndoAction::Rating { changes }
        | UndoAction::ColorLabel { changes }
        | UndoAction::Rotation { changes } => changes.len(),
        UndoAction::Favorite { changes } | UndoAction::Trashed { changes } => changes.len(),
        UndoAction::AlbumMembership { ids, .. } => ids.len(),
        other => other.affected_ids().len().max(1),
    }
}

/// Decodes `id` at `size` on the decoder pool and converts it there, so the
/// UI thread only stores the finished image.
fn request_frame(
    engine: &lumina_core::LuminaEngine,
    generation: u64,
    id: MediaId,
    size: u32,
    cx: &mut Context<LuminaApp>,
) {
    let (tx, rx) = async_channel::bounded(1);
    engine.request_thumbnail(id, size, move |frame| {
        let image = frame.and_then(|frame| match crate::bridge::frame_to_gpui_image(frame) {
            Ok(image) => Some(image),
            Err(err) => {
                log::warn!("compare frame for {id:?} could not be converted: {err:#}");
                None
            }
        });
        // A closed receiver means the app is gone; nothing to show.
        let _ = tx.send_blocking(image);
    });
    cx.spawn(async move |this, cx| {
        let Ok(image) = rx.recv().await else {
            return;
        };
        // The app may be gone; nothing to show then.
        let _ = this.update(cx, |app, cx| match image {
            Some(image) => app.land_compare_image(generation, id, image, cx),
            None => app.compare_decode_failed(generation, id, cx),
        });
    })
    .detach();
}
