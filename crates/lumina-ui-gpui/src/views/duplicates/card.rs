//! One copy in the compare pane: the image with its tags, the keep radio
//! and the facts.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaId, MediaItem};

use super::texture;
use crate::app::types::format_bytes;
use crate::i18n::{t, t_with};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// Cards share the width but never get narrower than this; past that the
/// row scrolls sideways.
const CARD_MIN_WIDTH: f32 = 180.0;

/// One copy: the image with its tag, BEST badge and resolution, the keep
/// radio, then its facts. The keeper has the accent outline; the selected
/// card is lifted to bg-3 with the focus ring instead.
pub(super) fn card(
    this: &LuminaApp,
    hash: &str,
    position: usize,
    item: &MediaItem,
    keeper: bool,
    cx: &mut Context<LuminaApp>,
) -> Stateful<Div> {
    let id = item.id.0;
    // The heuristic pick sorts first in its group.
    let best = position == 0;
    let selected = this.selected_item.as_ref().is_some_and(|i| i.id == item.id);
    let item_clone = item.clone();
    let tag = if best {
        t("duplicates.original")
    } else {
        t_with("duplicates.copy_n", &[&position.to_string()])
    };
    let format = std::path::Path::new(&item.file_name)
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    let source = match (item.make.as_deref(), item.model.as_deref()) {
        (Some(make), Some(model)) if model.starts_with(make) => model.to_string(),
        (Some(make), Some(model)) => format!("{make} {model}"),
        (Some(one), None) | (None, Some(one)) => one.to_string(),
        (None, None) => item.parent_dir.clone(),
    };
    let edits = if this.edits.edited_ids.contains(&item.id) {
        t("duplicates.edited")
    } else {
        t("duplicates.not_edited")
    };
    let resolution =
        (item.width > 0 && item.height > 0).then(|| format!("{} × {}", item.width, item.height));
    div()
        .id(("duplicate-candidate", id))
        .debug_selector(move || format!("duplicate-candidate-{id}"))
        .role(Role::Group)
        .aria_label(item.file_name.clone())
        .flex_1()
        .min_w(px(CARD_MIN_WIDTH))
        .rounded(px(12.0))
        .bg(if selected {
            theme::bg_3()
        } else {
            theme::bg_1()
        })
        .map(|card| {
            if keeper {
                card.border(px(1.5))
            } else {
                card.border_1()
            }
        })
        // The selection (what ⌫ and ⌘E act on) wins over the keeper's accent.
        .border_color(if selected {
            theme::focus_ring()
        } else if keeper {
            theme::accent()
        } else {
            theme::line()
        })
        .children(
            selected
                .then(|| text_marker("duplicate-selected", &id.to_string().into()))
                .flatten(),
        )
        .overflow_hidden()
        .flex()
        .flex_col()
        .cursor_pointer()
        .on_click(cx.listener(move |this, e: &ClickEvent, _w, cx| {
            if e.click_count() >= 2 {
                this.open_viewer(item_clone.clone(), cx);
            } else {
                this.selected_item = Some(item_clone.clone());
                this.selected_index = None;
                this.selected_indices.clear();
                this.refresh_selected_tags();
                cx.notify();
            }
        }))
        .child(
            div()
                .relative()
                .w_full()
                .h(px(400.0))
                .bg(theme::bg_media())
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .child(match texture(this, item) {
                    Some(tex) => img(tex)
                        .size_full()
                        .object_fit(ObjectFit::Contain)
                        .into_any_element(),
                    None => render_icon(Icon::Image, px(24.0), theme::t3()).into_any_element(),
                })
                .child(media_tag(tag).top(px(10.0)).left(px(10.0)))
                .when(best, |image| image.child(best_badge(id)))
                .children(resolution.map(|r| media_tag(r).bottom(px(10.0)).left(px(10.0)))),
        )
        .child(
            div()
                .p(px(14.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(keep_radio(hash, item.id, keeper, cx))
                .child(fact(
                    "duplicates.fact_size",
                    format_bytes(item.file_size),
                    false,
                ))
                .child(fact("duplicates.fact_format", format, false))
                .child(fact("duplicates.fact_edits", edits, false))
                .child(fact("duplicates.fact_source", source, false))
                .child(fact(
                    "duplicates.fact_captured",
                    item.formatted_timestamp(),
                    true,
                ))
                .child(fact(
                    "duplicates.fact_folder",
                    item.parent_dir.clone(),
                    false,
                )),
        )
}

/// The accent BEST badge, top right: the heuristic's pick.
fn best_badge(id: u64) -> Stateful<Div> {
    div()
        .id(("duplicate-best", id))
        .debug_selector(move || format!("duplicate-best-{id}"))
        .tooltip(theme::tooltip_label(t("duplicates.best_hint")))
        .absolute()
        .top(px(10.0))
        .right(px(10.0))
        .px(px(6.0))
        .py(px(2.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::accent())
        .font_family(theme::FONT_MONO)
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::on_accent())
        .child(t("duplicates.best"))
}

/// "Keep this copy": a 16 px radio of its own, so choosing the keeper does
/// not select the card.
fn keep_radio(hash: &str, id: MediaId, on: bool, cx: &mut Context<LuminaApp>) -> Stateful<Div> {
    let label = if on {
        t("duplicates.keeping_this")
    } else {
        t("duplicates.keep_this")
    };
    let hash = hash.to_string();
    let n = id.0;
    div()
        .id(("duplicate-keep", n))
        .debug_selector(move || format!("duplicate-keep-{n}"))
        .role(Role::RadioButton)
        .aria_label(label.clone())
        .aria_toggled(if on { Toggled::True } else { Toggled::False })
        .pb(px(4.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .cursor_pointer()
        .child(
            div()
                .size(px(16.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .map(|ring| {
                    if on {
                        ring.bg(theme::accent_soft())
                            .border_color(theme::accent())
                            .child(div().size(px(7.0)).rounded_full().bg(theme::accent()))
                    } else {
                        ring.bg(theme::bg_2()).border_color(theme::line_2())
                    }
                }),
        )
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if on { theme::accent() } else { theme::t2() })
                .child(label),
        )
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            cx.stop_propagation();
            this.stacks.review.keepers.insert(hash.clone(), id);
            cx.notify();
        }))
}

/// A mono label over the image, on the media scrim.
fn media_tag(text: impl Into<SharedString>) -> Div {
    div()
        .absolute()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(theme::RADIUS_SM)
        .bg(theme::media_scrim())
        .font_family(theme::FONT_MONO)
        .text_size(px(10.0))
        .text_color(theme::on_media())
        .child(text.into())
}

/// A key/value row; `dim` draws the value in t3 (Captured).
fn fact(key: &str, value: String, dim: bool) -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(11.5))
                .text_color(theme::t3())
                .child(t(key)),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .font_family(theme::FONT_MONO)
                .text_size(px(11.5))
                .text_color(if dim { theme::t3() } else { theme::t1() })
                .child(value),
        )
}
