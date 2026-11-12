//! Right column of the Tags page (frame `L3F8Cs`): the focused tag's name
//! with the pencil rename, PREVIEW, ALIASES, ORGANISE and Delete tag.
//! ALIASES and ORGANISE live in `detail_sections.rs`.

use crate::app::text_input::TextTarget;
use gpui::*;

use super::detail_sections::{render_aliases, render_organise};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;
use crate::LuminaApp;

/// The panel's width (frame `L3F8Cs`).
pub(crate) const WIDTH: f32 = 359.5;

/// PREVIEW grid: 2 rows of 4; `load_tag_manager_items` loads this many.
pub(crate) const PREVIEW_COLUMNS: usize = 4;
pub(crate) const PREVIEW_ITEMS: usize = 2 * PREVIEW_COLUMNS;

pub fn render_detail(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let panel = div()
        .id("tag-manager-detail")
        .debug_selector(|| "tag-manager-detail".to_string())
        .w(px(WIDTH))
        .flex_shrink_0()
        .bg(theme::bg_1())
        .border_l_1()
        .border_color(theme::line())
        .px(px(18.0))
        .py(px(20.0))
        .flex()
        .flex_col()
        .gap(px(20.0))
        .overflow_y_scroll();

    let Some(tag) = app.tag_manager.focused.clone() else {
        return panel
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(theme::t2())
            .child(t("tags.detail_empty"))
            .into_any_element();
    };
    // The keyword and everything below it, loaded on focus.
    let items_label = crate::i18n::items(app.tag_manager.focused_count);

    panel
        .child(render_header(app, &tag, &items_label, cx))
        .child(render_preview(app, &tag, cx))
        .child(render_aliases(app, &tag, cx))
        .child(render_organise(app, &items_label, cx))
        .child(div().h(px(1.0)).flex_none().bg(theme::line()))
        .child(render_delete(app, &tag, &items_label, cx))
        .into_any_element()
}

/// Dot 11, the name (Inter Tight 21/600) with the pencil, and "N items".
/// While renaming, the name is the rename field: ↵ saves, Esc cancels.
fn render_header(
    app: &LuminaApp,
    tag: &str,
    items_label: &str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let renaming = app.tag_manager.editing.as_deref() == Some(tag) && !app.tag_manager.moving;
    let name: AnyElement = if renaming {
        theme::field("tag-manager-rename-field")
            .debug_selector(|| "tag-manager-rename-field".to_string())
            .w_full()
            .h(px(32.0))
            .border_color(theme::focus_ring())
            .font_family(theme::FONT_DISPLAY)
            .text_size(px(21.0))
            .font_weight(FontWeight::SEMIBOLD)
            .children(app.field_input_for(TextTarget::TagManagerRename))
            .into_any_element()
    } else {
        div()
            .truncate()
            .font_family(theme::FONT_DISPLAY)
            .text_size(px(21.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::t1())
            .child(tag.to_string())
            .into_any_element()
    };
    let rename_tag = tag.to_string();
    let rename_label: SharedString = t("tags.rename").into();
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                // The colour hashed from the name (tags store no colour).
                .child(
                    div()
                        .size(px(11.0))
                        .flex_none()
                        .rounded_full()
                        .bg(crate::views::sidebar::tag_color(tag)),
                )
                .child(
                    div()
                        .id("tag-manager-detail-name")
                        .debug_selector(|| "tag-manager-detail-name".to_string())
                        .flex_1()
                        .min_w_0()
                        .child(name),
                )
                .child(
                    div()
                        .id("tag-detail-edit")
                        .debug_selector(|| "tag-detail-edit".to_string())
                        .role(Role::Button)
                        .aria_label(rename_label.clone())
                        .tooltip(theme::tooltip_label(rename_label))
                        .flex_none()
                        .p(px(4.0))
                        .rounded(theme::RADIUS_SM)
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::bg_2()))
                        .child(render_icon(Icon::Pencil, px(14.0), theme::t2()))
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.start_modal_tag_rename(rename_tag.clone(), cx);
                        })),
                ),
        )
        .child(
            div()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .text_color(theme::t3())
                .child(items_label.to_string()),
        )
        .children(renaming.then(|| render_touches(items_label)))
}

/// How many items a rename or move changes (`subtree_item_count`, loaded
/// when the keyword was focused).
pub(super) fn render_touches(items_label: &str) -> impl IntoElement {
    div()
        .id("tag-detail-touches")
        .debug_selector(|| "tag-detail-touches".to_string())
        .text_size(px(10.5))
        .text_color(theme::t3())
        .child(t("tags.touches").replace("{items}", items_label))
}

/// Section head: the label, and an optional trailing element.
pub(super) fn section_head(title: String) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(theme::section_label(title))
}

/// PREVIEW: "View all" (the library searched for the tag) and the newest
/// 8 items, 4 across, 68 high, from the grid's texture cache.
fn render_preview(app: &LuminaApp, tag: &str, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let show_tag = tag.to_string();
    let view_all: SharedString = t("tags.view_all").into();
    let head = section_head(t("tags.preview")).child(
        div()
            .id("tag-detail-view-all")
            .debug_selector(|| "tag-detail-view-all".to_string())
            .role(Role::Link)
            .aria_label(t("tags.view_all_label"))
            .tooltip(theme::tooltip_label(t("tags.view_all_label")))
            .cursor_pointer()
            .text_size(px(11.0))
            .text_color(theme::accent())
            .child(view_all)
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                // The search spans the library; ⌘[ comes back here.
                this.set_search_query(&crate::app::search::tag_search_suggestion(&show_tag), cx);
            })),
    );
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(head)
        .child(render_items(app))
}

fn render_items(app: &LuminaApp) -> AnyElement {
    let ids = &app.tag_manager.tagged_items;
    if ids.is_empty() {
        return div()
            .id("tag-manager-items-empty")
            .debug_selector(|| "tag-manager-items-empty".to_string())
            .text_xs()
            .text_color(theme::t2())
            .child(t("tags.no_items"))
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
        .grid_cols(PREVIEW_COLUMNS as u16)
        .gap(px(5.0))
        // ponytail: items the grid has not decoded show as placeholders;
        // request thumbnails here if that proves too sparse.
        .children(thumbs.into_iter().enumerate().map(|(i, thumb)| {
            div()
                .debug_selector(move || format!("tag-manager-item-{i}"))
                .h(px(68.0))
                .overflow_hidden()
                .rounded(theme::RADIUS_SM)
                .bg(theme::bg_2())
                .children(thumb.map(|image| img(image).size_full().object_fit(ObjectFit::Cover)))
        }))
        .into_any_element()
}

/// Full-width danger-soft button and the hint with the real item count.
fn render_delete(
    app: &LuminaApp,
    tag: &str,
    items_label: &str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    // ponytail: O(keywords) per frame, in memory; cache with the counts
    // refresh if keyword lists grow huge.
    let prefix = format!("{tag}/");
    let has_children = app
        .library_tag_counts
        .iter()
        .any(|(t, _)| t.starts_with(&prefix));
    let hint: SharedString = t(if has_children {
        "tags.delete_note_children"
    } else {
        "tags.delete_note"
    })
    .replace("{items}", items_label)
    .into();
    let delete_tag = tag.to_string();
    let delete_selector = format!("tag-manager-delete-{tag}");
    div()
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .id(ElementId::Name(format!("modal-del-btn-{tag}").into()))
                .debug_selector(move || delete_selector.clone())
                .role(Role::Button)
                .aria_label(format!("{}. {hint}", t("tags.delete")))
                .h(px(34.0))
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .rounded(theme::RADIUS_LG)
                .border_1()
                .border_color(theme::danger_border())
                .bg(theme::danger_soft())
                .hover(|s| s.bg(theme::danger_bg_hover()))
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::danger())
                .child(render_icon(Icon::Trash, px(13.0), theme::danger()))
                .child(t("tags.delete"))
                .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                    this.delete_tag_globally(delete_tag.clone(), cx);
                })),
        )
        .child(
            div()
                .id("tag-manager-delete-note")
                .debug_selector(|| "tag-manager-delete-note".to_string())
                .relative()
                .text_size(px(10.5))
                .text_color(theme::t3())
                .children(text_marker("tag-detail-delete-hint", &hint))
                .child(hint),
        )
}
