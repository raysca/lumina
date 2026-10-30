//! Batch inspector chips for tags on only some of the selected items.

use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

/// Tags on only some of the selection: dashed chips with an `n/total` count.
/// Clicking one adds it to every selected item; its × removes it from all.
pub(super) fn render_partial_tags(
    weak_app: WeakEntity<LuminaApp>,
    partial: &[(String, usize)],
    total: usize,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1p5()
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("On some items · click to apply to all, × to remove from all"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(partial.iter().map(|(tag, n)| {
                    let app = weak_app.clone();
                    let remove_app = weak_app.clone();
                    let tag_for_click = tag.clone();
                    let tag_for_remove = tag.clone();
                    let selector = format!("batch-partial-tag-{tag}");
                    let remove_selector = format!("batch-partial-tag-remove-{tag}");
                    div()
                        .id(ElementId::Name(selector.clone().into()))
                        .debug_selector(move || selector.clone())
                        .tooltip(theme::tooltip_label(format!(
                            "On {n} of {total} · add to all"
                        )))
                        .h(px(24.0))
                        .px(px(9.0))
                        .rounded_full()
                        .border_1()
                        .border_dashed()
                        .border_color(theme::text_disabled())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(tag.clone())
                        .child(
                            div()
                                .font_family(theme::FONT_MONO)
                                .text_size(px(10.0))
                                .text_color(theme::text_subtle())
                                .child(format!("{n}/{total}")),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(remove_selector.clone().into()))
                                .debug_selector(move || remove_selector.clone())
                                .tooltip(theme::tooltip_label("Remove from all"))
                                .rounded_full()
                                .hover(|s| s.bg(theme::border_strong()))
                                .child(render_icon(Icon::X, px(10.0), theme::text_subtle()))
                                .on_click(move |_e, _w, cx| {
                                    // The chip itself would otherwise add the tag.
                                    cx.stop_propagation();
                                    if let Some(app) = remove_app.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.batch_remove_tag(tag_for_remove.clone(), cx)
                                        });
                                    }
                                }),
                        )
                        .on_click(move |_e, _w, cx| {
                            if let Some(app) = app.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_add_tag(tag_for_click.clone(), cx)
                                });
                            }
                        })
                })),
        )
        .into_any_element()
}
