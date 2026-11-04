//! Right column of the tag manager: the focused tag's name, items and actions.

use crate::app::caret::caret;
use gpui::*;

use crate::theme;
use crate::LuminaApp;

/// TAGGED ITEMS grid columns.
const GRID_COLUMNS: usize = 6;

pub fn render_detail(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let panel = div()
        .id("tag-manager-detail")
        .debug_selector(|| "tag-manager-detail".to_string())
        .flex_1()
        .min_w(px(0.0))
        .bg(theme::bg_pane())
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(20.0));

    let Some(tag) = app.tag_manager.focused.clone() else {
        return panel
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(theme::text_subtle())
            .child("Select a tag to see its details.")
            .into_any_element();
    };
    let count = app
        .library_tag_counts
        .iter()
        .find(|(t, _)| *t == tag)
        .map_or(0, |(_, n)| *n);
    let items_label = format!("{count} item{}", if count == 1 { "" } else { "s" });

    panel
        .child(render_header(&tag, &items_label, cx))
        .child(section("NAME", render_name_field(app, &tag, cx)))
        .child(section("TAGGED ITEMS", render_items(app)))
        .child(div().flex_1())
        .child({
            let delete_tag = tag.clone();
            let delete_selector = format!("tag-manager-delete-{tag}");
            div()
                .pt(px(14.0))
                .border_t_1()
                .border_color(theme::border_subtle())
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(
                    theme::control(ElementId::Name(format!("modal-del-btn-{tag}").into()))
                        .debug_selector(move || delete_selector.clone())
                        .text_color(theme::danger_text())
                        .child("Delete Tag")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.delete_tag_globally(delete_tag.clone(), cx);
                        })),
                )
                .child(
                    div()
                        .id("tag-manager-delete-note")
                        .debug_selector(|| "tag-manager-delete-note".to_string())
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child(format!(
                            "Removes the tag from {items_label}. The photos stay."
                        )),
                )
        })
        .into_any_element()
}

/// Dot, 24px name, "N items", and Show in Browser on the right.
fn render_header(tag: &str, items_label: &str, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let show_tag = tag.to_string();
    let selector = format!("tag-manager-show-{tag}");
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        // ponytail: one neutral dot until tags have colours (schema change).
        .child(
            div()
                .size(px(10.0))
                .flex_none()
                .rounded_full()
                .bg(theme::text_muted()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .id("tag-manager-detail-name")
                        .debug_selector(|| "tag-manager-detail-name".to_string())
                        .truncate()
                        .text_size(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text())
                        .child(tag.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(items_label.to_string()),
                ),
        )
        .child(
            theme::control(ElementId::Name(
                format!("modal-tag-name-filter-{tag}").into(),
            ))
            .debug_selector(move || selector.clone())
            .child("Show in Browser")
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                this.set_search_query(&format!("tag:{show_tag}"), cx);
                this.close_tag_manager(cx);
            })),
        )
}

fn section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(theme::section_label(title))
        .child(content)
}

/// Always shown as a field: clicking (or ↵) edits in place, ↵ saves, Esc
/// cancels. The keymap feeds `editing_buffer`.
fn render_name_field(app: &LuminaApp, tag: &str, cx: &mut Context<LuminaApp>) -> AnyElement {
    let editing = app.tag_manager.editing.as_deref() == Some(tag);
    let rename_tag = tag.to_string();
    theme::field("tag-manager-rename-field")
        .debug_selector(|| "tag-manager-rename-field".to_string())
        .tooltip(theme::tooltip_label("Rename (↵)"))
        .cursor_text()
        .border_color(if editing {
            theme::focus_ring()
        } else {
            theme::border()
        })
        .text_color(theme::text())
        .child(if editing {
            format!("{}{}", app.tag_manager.editing_buffer, caret())
        } else {
            tag.to_string()
        })
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            if this.tag_manager.editing.is_none() {
                this.start_modal_tag_rename(rename_tag.clone(), cx);
            }
        }))
        .into_any_element()
}

/// Six-column thumbnails of the newest tagged items, from the grid's cache.
fn render_items(app: &LuminaApp) -> AnyElement {
    let ids = &app.tag_manager.tagged_items;
    if ids.is_empty() {
        return div()
            .id("tag-manager-items-empty")
            .debug_selector(|| "tag-manager-items-empty".to_string())
            .text_xs()
            .text_color(theme::text_subtle())
            .child("No items carry this tag.")
            .into_any_element();
    }
    let thumbs: Vec<Option<std::sync::Arc<RenderImage>>> = match &app.grid_state {
        Some(grid) => {
            let cache = grid.texture_cache.lock();
            ids.iter().map(|id| cache.peek(id).cloned()).collect()
        }
        None => vec![None; ids.len()],
    };
    div()
        .id("tag-manager-items")
        .debug_selector(|| "tag-manager-items".to_string())
        .grid()
        .grid_cols(GRID_COLUMNS as u16)
        .gap(px(6.0))
        // ponytail: items the grid has not decoded show as placeholders;
        // request thumbnails here if that proves too sparse.
        .children(thumbs.into_iter().map(|thumb| {
            div()
                .h(px(64.0))
                .overflow_hidden()
                .bg(theme::bg_control())
                .children(thumb.map(|image| img(image).size_full().object_fit(ObjectFit::Cover)))
        }))
        .into_any_element()
}
