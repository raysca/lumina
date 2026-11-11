//! The viewer's filmstrip (Design 02 "Detail"): a bottom band of the items
//! around the current one. It reads only the cached view rows and the grid's
//! texture cache, so a frame never queries SQLite, and it renders a bounded
//! window however large the view is.

use std::ops::Range;

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaId, MediaItem};

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// Most thumbs the strip renders: 7 either side of the current one.
pub(crate) const FILMSTRIP_MAX: usize = 15;
/// Strip height, its top border included.
pub(crate) const FILMSTRIP_HEIGHT: f32 = 95.5;
const THUMB: f32 = 58.0;
const CURRENT: f32 = 74.0;
const GAP: f32 = 6.0;
const PAD_X: f32 = 20.0;
const THUMB_RADIUS: f32 = 3.0;

/// Whether the viewer shows the strip: an item at a view position, and not
/// immersive, in a slideshow or in edit mode.
pub(crate) fn filmstrip_shown(this: &LuminaApp) -> bool {
    this.selected_index.is_some()
        && this.grid_state.is_some()
        && !this.is_immersive_fullscreen
        && this.slideshow.is_none()
        && this.edits.session.is_none()
}

/// How many thumbs fit a stage `width` wide (odd, so the current thumb can
/// sit in the middle), at most [`FILMSTRIP_MAX`]. `None` (bounds not known
/// yet) gives the maximum.
pub(crate) fn filmstrip_slots(width: Option<f32>) -> usize {
    let Some(width) = width else {
        return FILMSTRIP_MAX;
    };
    let room = (width - PAD_X * 2.0 - CURRENT).max(0.0);
    let per_side = ((room / (THUMB + GAP)) as usize) / 2;
    (1 + per_side * 2).min(FILMSTRIP_MAX)
}

/// The indices to show: `slots` around `current`, shifted inward at the
/// ends of the view so no slot is empty and nothing wraps.
pub(crate) fn filmstrip_window(current: usize, count: usize, slots: usize) -> Range<usize> {
    let len = slots.min(count);
    let start = current
        .saturating_sub(len / 2)
        .min(count.saturating_sub(len));
    start..start + len
}

/// The strip, or nothing when it is hidden or the view has no position.
pub(crate) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Option<AnyElement> {
    if !filmstrip_shown(this) {
        return None;
    }
    let (Some(current), Some(state)) = (this.selected_index, this.grid_state.as_ref()) else {
        return None;
    };
    let window = filmstrip_window(
        current,
        this.displayed_count(),
        filmstrip_slots(super::stage_width(this)),
    );
    // Cached rows only; until the view's fetch covers the window the strip
    // shows what it has.
    // The current thumb is matched by id: for one fetch after a trash or an
    // edit the rows can be stale while `selected_index` is already remapped.
    let selected = this.selected_item.as_ref().map(|item| item.id);
    let rows = this.displayed_rows(window.clone());
    let thumbs = rows.iter().zip(window.start..).map(|(item, index)| {
        let texture = state.texture_cache.lock().get(&item.id).cloned();
        if texture.is_none() {
            // Skips ids already in flight or failed, so stepping queues
            // only the thumb entering the strip.
            this.queue_thumbnail_request_item(item);
        }
        let failed = this.missing_items.contains(&item.id) || state.is_failed(item.id);
        thumb(item, index, selected == Some(item.id), texture, failed, cx)
    });
    Some(
        div()
            .id("viewer-filmstrip")
            .debug_selector(|| "viewer-filmstrip".to_string())
            .flex_none()
            .w_full()
            .h(px(FILMSTRIP_HEIGHT))
            .border_t_1()
            .border_color(theme::line())
            .px(px(PAD_X))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(GAP))
            .overflow_hidden()
            .children(thumbs)
            .into_any_element(),
    )
}

fn thumb(
    item: &MediaItem,
    index: usize,
    is_current: bool,
    texture: Option<std::sync::Arc<RenderImage>>,
    failed: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let selector = if is_current {
        "viewer-filmstrip-current".to_string()
    } else {
        format!("viewer-filmstrip-thumb-{}", item.id.0)
    };
    let side = if is_current { CURRENT } else { THUMB };
    let id = item.id.0;
    let item_id = item.id;
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .role(Role::Button)
        .aria_label(item.file_name.clone())
        .tooltip(theme::tooltip_label(item.file_name.clone()))
        .flex_none()
        .relative()
        .size(px(side))
        .rounded(px(THUMB_RADIUS))
        .overflow_hidden()
        .bg(theme::bg_2())
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .map(|thumb| match texture {
            Some(texture) => thumb.child(
                img(texture)
                    .debug_selector(move || format!("viewer-filmstrip-img-{id}"))
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            ),
            // Missing on disk or undecodable: a glyph instead of a picture.
            None if failed => thumb.child(
                div()
                    .debug_selector(move || format!("viewer-filmstrip-failed-{id}"))
                    .child(render_icon(Icon::Image, px(16.0), theme::t3())),
            ),
            // Still decoding: the plain well.
            None => thumb,
        })
        .map(|thumb| {
            if is_current {
                thumb.border_2().border_color(theme::accent())
            } else {
                // Frame: the other thumbs show their picture at 55%.
                thumb.opacity(0.55)
            }
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            if !is_current {
                this.open_filmstrip_item(item_id, index, cx);
            }
        }))
}

impl LuminaApp {
    /// A thumb click: opens exactly that item. `near` is where the strip
    /// drew it, which can be one fetch stale, so its position is looked up
    /// again in fresh navigation rows (one bounded read); an item that left
    /// the view meanwhile opens nothing.
    fn open_filmstrip_item(&mut self, id: MediaId, near: usize, cx: &mut Context<Self>) {
        let lo = near.saturating_sub(FILMSTRIP_MAX);
        let Some(&index) = self.positions_in(&[id], lo, near + FILMSTRIP_MAX).get(&id) else {
            return;
        };
        self.viewer_state.reset();
        self.select_index(index, cx);
    }
}
