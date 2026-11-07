//! Edit mode in the viewer (E, ⌘K Edit Photo, Image > Edit Photo): rotate,
//! exposure and auto-enhance on a draft, saved on Done as one undo step;
//! Revert to Original for the selection; and the cached set of edited ids
//! behind the grid badge. Keys are in `keymap/modals.rs`, the bar in
//! `views/viewer/edit_bar.rs`.

use std::collections::HashSet;
use std::sync::Arc;

use gpui::*;
use lumina_core::{DecodedFrame, Edits, MediaId, MediaType};

use crate::app::undo::UndoAction;
use crate::app::LuminaApp;
use crate::views::viewer_state::ViewerRotation;

/// Exposure moves in thirds of a stop, up to two stops either way.
const EXPOSURE_STEPS_PER_EV: f32 = 3.0;
const MAX_EXPOSURE_STEPS: i32 = 6;

/// The photo being edited in the viewer.
pub struct QuickEditState {
    pub item: MediaId,
    /// What Done writes.
    pub draft: Edits,
    /// The catalog's edits when edit mode opened; the undo "before".
    pub original: Edits,
    /// The unedited decode the preview is rendered from (the viewer frame
    /// already has the saved edits). `None` until it arrives, or if the
    /// decode failed; the viewer then keeps the saved frame.
    pub source: Option<Arc<DecodedFrame>>,
    /// Bumped by each preview render; only the newest result is shown.
    pub(crate) generation: u64,
    /// The draft the one render in flight was started with. Changes made
    /// meanwhile (a held key) wait, then one more render covers them all.
    pub(crate) rendering: Option<Edits>,
    /// Renders started in this session, and the draft the preview on screen
    /// shows, for the coalescing test.
    #[cfg(test)]
    pub(crate) renders: u32,
    #[cfg(test)]
    pub(crate) shown: Option<Edits>,
}

/// Edit mode plus the edited-ids cache the grid badge reads every frame.
#[derive(Default)]
pub struct QuickEdits {
    pub session: Option<QuickEditState>,
    /// Items with an edit or a rotation. Loaded off the UI thread on library
    /// open and after every edit or rotation write, never per frame.
    pub edited_ids: HashSet<MediaId>,
    edited_ids_request: u64,
    /// Sessions opened so far; each session's render generations start in
    /// their own range, so a late render from a cancelled session never
    /// matches the next one's.
    sessions_opened: u64,
}

impl QuickEdits {
    /// A library switch: drops the draft and the badges, but keeps the
    /// request counter, so a late load from the old library is ignored.
    pub(crate) fn reset_for_library(&mut self) {
        self.session = None;
        self.edited_ids.clear();
    }
}

impl LuminaApp {
    /// Opens edit mode on the viewer's photo (from the grid, the viewer opens
    /// first). Refused for videos, behind sheets, in a field and in Recently
    /// Deleted; inside edit mode it does nothing.
    pub fn enter_edit_mode(&mut self, cx: &mut Context<Self>) {
        if self.edits.session.is_some() || self.media_edit_refused(("editing", "edit"), cx) {
            return;
        }
        let Some(item) = self.selected_item.clone() else {
            self.set_last_action("Select a photo to edit");
            cx.notify();
            return;
        };
        if item.media_type == MediaType::Video {
            self.set_last_action("Editing is not supported for video");
            cx.notify();
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before editing", cx);
            return;
        };
        let original = match engine.get_edits(&[item.id]) {
            Ok(mut edits) => edits.remove(&item.id).unwrap_or_default(),
            Err(err) => {
                self.set_error(format!("Could not read edits: {err:#}"), cx);
                return;
            }
        };
        if !self.is_fullscreen_viewer {
            self.open_viewer(item.clone(), cx);
        }
        self.context_menu_state = None;
        let id = item.id;
        self.edits.sessions_opened += 1;
        self.edits.session = Some(QuickEditState {
            item: id,
            draft: original,
            original,
            source: None,
            generation: self.edits.sessions_opened << 32,
            rendering: None,
            #[cfg(test)]
            renders: 0,
            #[cfg(test)]
            shown: None,
        });
        let (tx, rx) = async_channel::bounded(1);
        engine.request_unedited_frame(
            id,
            crate::app::viewer_actions::VIEWER_IMAGE_SIZE,
            move |frame| {
                // A closed receiver means edit mode is gone; nothing to show.
                let _ = tx.send_blocking(frame);
            },
        );
        // A decode that never answers (a hung native tool is cut off by the
        // decoder's timeout) leaves the saved frame on screen; Done still
        // saves the draft.
        cx.spawn(async move |this, cx| {
            if let Ok(frame) = rx.recv().await {
                // The app may be gone; nothing to show then.
                let _ = this.update(cx, |app, cx| app.land_edit_source(id, frame, cx));
            }
        })
        .detach();
        self.set_last_action("Editing photo: Enter saves, Esc cancels");
        cx.notify();
    }

    pub(crate) fn land_edit_source(
        &mut self,
        id: MediaId,
        frame: Option<DecodedFrame>,
        cx: &mut Context<Self>,
    ) {
        // The first answer wins; a repeat request's frame is the same one.
        let Some(session) = self
            .edits
            .session
            .as_mut()
            .filter(|s| s.item == id && s.source.is_none())
        else {
            return;
        };
        match frame {
            Some(frame) => {
                session.source = Some(Arc::new(frame));
                self.render_edit_preview(cx);
            }
            None => self.show_toast("Preview unavailable; Done still saves", cx),
        }
    }

    /// Renders the draft from the unedited source on the background
    /// executor, one render at a time: while one is in flight, changes wait
    /// and `land_edit_preview` starts one more for the latest draft.
    fn render_edit_preview(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.edits.session.as_mut() else {
            return;
        };
        let Some(source) = session.source.clone() else {
            return;
        };
        if session.rendering.is_some() {
            return;
        }
        session.generation += 1;
        session.rendering = Some(session.draft);
        #[cfg(test)]
        {
            session.renders += 1;
        }
        let (item, generation, draft) = (session.item, session.generation, session.draft);
        let task = cx.background_executor().spawn(async move {
            let frame = DecodedFrame {
                id: source.id,
                width: source.width,
                height: source.height,
                format: source.format,
                data: source.data.clone(),
            };
            crate::bridge::frame_to_gpui_image(lumina_core::edits::apply_edits(frame, &draft))
                .map_err(|err| log::warn!("edit preview for {item:?} failed: {err:#}"))
                .ok()
        });
        cx.spawn(async move |this, cx| {
            let image = task.await;
            // The app may be gone; nothing to show then.
            let _ = this.update(cx, |app, cx| {
                app.land_edit_preview(item, generation, image, cx)
            });
        })
        .detach();
    }

    /// Shows a rendered preview if it is the newest for the open session,
    /// then renders again if the draft changed meanwhile. A stale one was
    /// never painted, so it is simply dropped.
    pub(crate) fn land_edit_preview(
        &mut self,
        item: MediaId,
        generation: u64,
        image: Option<Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self
            .edits
            .session
            .as_mut()
            .filter(|s| s.item == item && s.generation == generation)
        else {
            return;
        };
        let rendered = session.rendering.take();
        let behind = rendered != Some(session.draft);
        #[cfg(test)]
        if image.is_some() {
            session.shown = rendered;
        }
        if let Some(image) = image {
            self.set_viewer_image(Some(image));
            cx.notify();
        }
        if behind {
            self.render_edit_preview(cx);
        }
    }

    fn change_draft(&mut self, change: impl FnOnce(&mut Edits), cx: &mut Context<Self>) {
        let Some(session) = self.edits.session.as_mut() else {
            return;
        };
        change(&mut session.draft);
        self.render_edit_preview(cx);
        cx.notify();
    }

    pub fn rotate_edit_draft(&mut self, direction: ViewerRotation, cx: &mut Context<Self>) {
        self.change_draft(|d| d.rotation = direction.apply_to(d.rotation), cx);
    }

    /// One third of a stop brighter (`1`) or darker (`-1`), within ±2 EV.
    pub fn step_edit_exposure(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.change_draft(
            |d| {
                let steps = (d.exposure_ev * EXPOSURE_STEPS_PER_EV).round() as i32 + delta;
                let steps = steps.clamp(-MAX_EXPOSURE_STEPS, MAX_EXPOSURE_STEPS);
                d.exposure_ev = steps as f32 / EXPOSURE_STEPS_PER_EV;
            },
            cx,
        );
    }

    pub fn toggle_edit_auto(&mut self, cx: &mut Context<Self>) {
        self.change_draft(|d| d.auto_enhance = !d.auto_enhance, cx);
    }

    /// Resets the draft to the original file (no rotation, no edits); Done
    /// saves that, Cancel keeps what was saved before.
    pub fn revert_edit_draft(&mut self, cx: &mut Context<Self>) {
        self.change_draft(|d| *d = Edits::default(), cx);
    }

    /// Escape / Cancel: leaves the catalog alone and shows the saved frame.
    pub fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.edits.session.take() else {
            return;
        };
        self.show_saved_frame(session.item);
        self.set_last_action("Edits discarded");
        cx.notify();
    }

    /// Enter / Done: writes the draft once, as one undo step. On failure
    /// edit mode stays open with the draft and the banner says why.
    pub fn finish_edit(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.edits.session.as_ref() else {
            return;
        };
        let (id, before, after) = (session.item, session.original, session.draft);
        if before == after {
            // Nothing to save; a preview on screen already shows these edits.
            self.edits.session = None;
            cx.notify();
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before editing", cx);
            return;
        };
        if let Err(err) = engine.set_edits(&[(id, after)]) {
            self.set_error(format!("Could not save edits: {err:#}"), cx);
            return;
        }
        self.edits.session = None;
        self.push_undo(UndoAction::Edits {
            changes: vec![(id, before, after)],
        });
        // The preview shows exactly what was saved: keep it on screen until
        // the re-requested saved frame replaces it, instead of blanking.
        let preview = self.viewer_image.clone();
        self.after_edits_written(&[id], cx);
        if preview.is_some() && self.viewer_image.is_none() {
            self.set_viewer_image(preview);
        }
        self.set_last_action("Saved edits");
        self.show_undo_toast("Saved edits", cx);
        cx.notify();
    }

    /// Revert to Original for the viewer photo or the grid selection, one
    /// undo step; inside edit mode it resets the draft instead.
    pub fn revert_selection_to_original(&mut self, cx: &mut Context<Self>) {
        if self.media_edit_refused(("reverting", "revert"), cx) {
            return;
        }
        if self.edits.session.is_some() {
            self.revert_edit_draft(cx);
            return;
        }
        let items = if self.is_fullscreen_viewer {
            self.selected_item.iter().cloned().collect()
        } else {
            self.get_selected_items()
        };
        let ids: Vec<MediaId> = items
            .iter()
            .filter(|item| item.media_type != MediaType::Video)
            .map(|item| item.id)
            .collect();
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before reverting", cx);
            return;
        };
        let current = match engine.get_edits(&ids) {
            Ok(current) => current,
            Err(err) => {
                self.set_error(format!("Could not read edits: {err:#}"), cx);
                return;
            }
        };
        let changes: Vec<(MediaId, Edits, Edits)> = ids
            .iter()
            .filter_map(|id| current.get(id).map(|&edits| (*id, edits, Edits::default())))
            .filter(|(_, before, after)| before != after)
            .collect();
        if changes.is_empty() {
            self.set_last_action("Nothing to revert");
            cx.notify();
            return;
        }
        let writes: Vec<(MediaId, Edits)> = changes.iter().map(|&(id, _, a)| (id, a)).collect();
        if let Err(err) = engine.set_edits(&writes) {
            self.set_error(format!("Could not revert: {err:#}"), cx);
            return;
        }
        let changed: Vec<MediaId> = writes.iter().map(|&(id, _)| id).collect();
        self.push_undo(UndoAction::Edits { changes });
        self.after_edits_written(&changed, cx);
        let message = match changed.len() {
            1 => "Reverted to original".to_string(),
            n => format!("Reverted {n} items to original"),
        };
        self.set_last_action(message.clone());
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    /// Drops what was painted with the old edits and reloads the badge set:
    /// after Done, Revert, rotation, and their undo and redo.
    pub(crate) fn after_edits_written(&mut self, ids: &[MediaId], cx: &mut Context<Self>) {
        self.evict_rendered(ids);
        self.reload_edited_ids(cx);
    }

    /// Reloads the edited-ids cache off the UI thread (about 8 ms at 100k
    /// items). A newer load, or a library switch, wins over a late result.
    pub(crate) fn reload_edited_ids(&mut self, cx: &mut Context<Self>) {
        self.edits.edited_ids_request += 1;
        let request = self.edits.edited_ids_request;
        let Some(engine) = self
            .grid_state
            .as_ref()
            .map(|state| Arc::downgrade(&state.engine))
        else {
            self.edits.edited_ids.clear();
            return;
        };
        // Weak: a queued load must not keep a closed library's engine (and
        // its event thread, which `open_library` joins) alive.
        let task = cx.background_executor().spawn(async move {
            engine
                .upgrade()
                .map(|engine| engine.edited_ids())
                .unwrap_or_else(|| Ok(HashSet::new()))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // The app may be gone; nothing to show then.
            let _ = this.update(cx, |app, cx| match result {
                Ok(ids) if app.edits.edited_ids_request == request => {
                    app.edits.edited_ids = ids;
                    cx.notify();
                }
                Ok(_) => {}
                Err(err) => log::warn!("could not read edited items: {err:#}"),
            });
        })
        .detach();
    }

    /// Drops an open draft without saving (the viewer closes, another photo
    /// is shown, a library opens) and says so, as Cancel does.
    pub(crate) fn discard_edit_draft(&mut self) {
        if self.edits.session.take().is_some() {
            self.set_last_action("Edits discarded");
        }
    }

    /// Replaces a preview with the catalog's (saved) frame of `id`.
    fn show_saved_frame(&mut self, id: MediaId) {
        self.set_viewer_image(None);
        if let Some(state) = self.grid_state.as_ref() {
            self.request_viewer_frame(state, id);
        }
    }
}
