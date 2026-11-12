//! The detail panel's ALIASES and ORGANISE sections (frame `L3F8Cs`).

use crate::app::text_input::TextTarget;
use gpui::*;

use super::detail::{render_touches, section_head};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

/// Alias chips (× removes) and "+ Add", which opens the alias field in its
/// place (also ⇧⌘A): ↵ adds, ⌫ in the empty field removes the last alias.
pub(super) fn render_aliases(
    app: &LuminaApp,
    tag: &str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let chips = app.aliases_of(tag).into_iter().map(|alias| {
        let chip_selector = format!("tag-detail-alias-{alias}");
        let remove_selector = format!("tag-detail-alias-remove-{alias}");
        let label = t("tags.alias_remove").replace("{alias}", &alias);
        let remove = alias.clone();
        chip(ElementId::Name(chip_selector.clone().into()))
            .debug_selector(move || chip_selector.clone())
            .bg(theme::bg_2())
            .text_color(theme::t2())
            .child(render_icon(Icon::CornerDownRight, px(13.0), theme::t3()))
            .child(alias)
            .child(
                div()
                    .id(ElementId::Name(remove_selector.clone().into()))
                    .debug_selector(move || remove_selector.clone())
                    .role(Role::Button)
                    .aria_label(label.clone())
                    .tooltip(theme::tooltip_label(label))
                    .cursor_pointer()
                    .child(render_icon(Icon::X, px(10.0), theme::t3()))
                    .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                        this.remove_keyword_alias(remove.clone(), cx);
                    })),
            )
    });
    let add: AnyElement = if app.tag_manager.alias_editing {
        chip("tag-detail-alias-input")
            .debug_selector(|| "tag-detail-alias-input".to_string())
            .w(px(140.0))
            .border_1()
            .border_color(theme::focus_ring())
            .text_color(theme::t1())
            .cursor_text()
            .children(app.field_input_for(TextTarget::TagManagerAlias))
            .into_any_element()
    } else {
        let label: SharedString = t("tags.alias_add_tooltip").into();
        chip("tag-detail-alias-add")
            .debug_selector(|| "tag-detail-alias-add".to_string())
            .role(Role::Button)
            .aria_label(label.clone())
            .tooltip(theme::tooltip_label(label))
            .border_1()
            .border_color(theme::line_2())
            .text_color(theme::t3())
            .cursor_pointer()
            .hover(|s| s.bg(theme::bg_2()))
            .child(render_icon(Icon::Plus, px(12.0), theme::t3()))
            .child(t("tags.alias_add"))
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.start_alias_input(cx);
            }))
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(section_head(t("tags.aliases")))
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme::t3())
                .child(t("tags.aliases_hint").replace("{tag}", tag)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(6.0))
                .children(chips)
                .child(add),
        )
}

/// A 28 px pill: 11 px side padding, 6 px gap, Inter 12.
fn chip(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .px(px(11.0))
        .rounded_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .text_size(px(12.0))
}

/// ORGANISE: Move to… as a row (⇧⌘M), and its parent field while moving.
pub(super) fn render_organise(
    app: &LuminaApp,
    items_label: &str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let label: SharedString = t("tags.row_move").into();
    let sub: SharedString = t("tags.move_sub").into();
    let moving = app.tag_manager.moving;
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(section_head(t("tags.organise")))
        .child(
            div()
                .id("tag-detail-move-btn")
                .debug_selector(|| "tag-detail-move".to_string())
                .role(Role::Button)
                .aria_label(format!("{label} {sub}"))
                .h(px(46.0))
                .px(px(11.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .rounded(theme::RADIUS_LG)
                .border_1()
                .border_color(theme::line())
                .bg(theme::bg_0())
                .hover(|s| s.bg(theme::bg_2()))
                .cursor_pointer()
                .child(render_icon(Icon::CornerDownRight, px(15.0), theme::t2()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::t1())
                                .child(label),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(10.0))
                                .text_color(theme::t3())
                                .child(sub),
                        ),
                )
                .child(render_icon(Icon::ChevronRight, px(14.0), theme::t3()))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.start_keyword_move(cx);
                })),
        )
        .children(moving.then(|| {
            theme::field("tag-detail-move-field")
                .debug_selector(|| "tag-detail-move-field".to_string())
                .border_color(theme::focus_ring())
                .children(app.field_input_for(TextTarget::TagManagerRename))
        }))
        .children(moving.then(|| {
            div()
                .text_size(px(10.5))
                .text_color(theme::t3())
                .child(t("tags.move_hint"))
        }))
        .children(moving.then(|| render_touches(items_label)))
}
