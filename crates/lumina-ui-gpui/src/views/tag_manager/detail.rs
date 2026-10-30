//! Right column of the tag manager: the focused tag's name, usage and actions.

use gpui::*;

use crate::theme;
use crate::LuminaApp;

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
    let editing = app.tag_manager.editing.as_deref() == Some(tag.as_str());

    panel
        .child(section(
            "NAME",
            if editing {
                render_rename_field(app, &tag, cx)
            } else {
                let rename_tag = tag.clone();
                let selector = format!("tag-manager-rename-{tag}");
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::text())
                            .child(tag.clone()),
                    )
                    .child(
                        theme::control(ElementId::Name(format!("modal-rename-btn-{tag}").into()))
                            .debug_selector(move || selector.clone())
                            .tooltip(theme::tooltip_label("Rename (↵)"))
                            .child("Rename")
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.start_modal_tag_rename(rename_tag.clone(), cx);
                            })),
                    )
                    .into_any_element()
            },
        ))
        .child(section("TAGGED ITEMS", {
            let show_tag = tag.clone();
            let selector = format!("tag-manager-show-{tag}");
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .text_color(theme::text_muted())
                        .child(format!("{count} item{}", if count == 1 { "" } else { "s" })),
                )
                .child(
                    theme::control(ElementId::Name(
                        format!("modal-tag-name-filter-{tag}").into(),
                    ))
                    .debug_selector(move || selector.clone())
                    .child("Show in Library")
                    .on_click(cx.listener(
                        move |this, _e: &ClickEvent, _w, cx| {
                            this.set_search_query(&format!("tag:{show_tag}"), cx);
                            this.close_tag_manager(cx);
                        },
                    )),
                )
        }))
        .child(div().flex_1())
        .child({
            let (merge_tag, delete_tag) = (tag.clone(), tag.clone());
            let merge_selector = format!("tag-manager-merge-{tag}");
            let delete_selector = format!("tag-manager-delete-{tag}");
            div()
                .pt(px(14.0))
                .border_t_1()
                .border_color(theme::border_subtle())
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    theme::control(ElementId::Name(format!("modal-merge-btn-{tag}").into()))
                        .debug_selector(move || merge_selector.clone())
                        .tooltip(theme::tooltip_label(
                            "Move this tag's items onto another tag, then remove it",
                        ))
                        .child("Merge into…")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.start_tag_merge(merge_tag.clone(), cx);
                        })),
                )
                .child(div().flex_1())
                .child(
                    theme::control(ElementId::Name(format!("modal-del-btn-{tag}").into()))
                        .debug_selector(move || delete_selector.clone())
                        .text_color(theme::danger_text())
                        .child("Delete Tag")
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.delete_tag_globally(delete_tag.clone(), cx);
                        })),
                )
        })
        .into_any_element()
}

fn section(title: &'static str, content: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text_subtle())
                .child(title),
        )
        .child(content)
}

/// Inline rename: the keymap feeds `editing_buffer`; ↵ saves, Esc cancels.
fn render_rename_field(app: &LuminaApp, tag: &str, cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            theme::field("tag-manager-rename-field")
                .debug_selector(|| "tag-manager-rename-field".to_string())
                .flex_1()
                .border_color(theme::accent())
                .child(format!("{}|", app.tag_manager.editing_buffer)),
        )
        .child(
            theme::control_primary(ElementId::Name(format!("modal-save-rename-{tag}").into()))
                .child("Save")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.commit_modal_tag_rename(cx);
                })),
        )
        .child(
            theme::control(ElementId::Name(format!("modal-cancel-rename-{tag}").into()))
                .child("Cancel")
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.cancel_modal_tag_rename(cx);
                })),
        )
        .into_any_element()
}
