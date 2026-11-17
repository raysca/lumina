//! The detail panel's ALIASES and ORGANISE sections.

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

/// ORGANISE: Merge into… (its target picker while open), Save as Smart
/// Album and Move to… (⇧⌘M, its parent field while moving), as rows.
pub(super) fn render_organise(
    app: &LuminaApp,
    items_label: &str,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    let moving = app.tag_manager.moving;
    let merge = organise_row(
        "tag-detail-merge",
        Icon::Merge,
        t("tags.row_merge"),
        t("tags.merge_sub"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.start_tag_merge(cx);
    }));
    let save_smart = organise_row(
        "tag-detail-save-smart",
        Icon::FolderPlus,
        t("tags.row_save_smart"),
        t("tags.save_smart_sub"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.save_command_keyword_as_album(cx);
    }));
    let move_row = organise_row(
        "tag-detail-move",
        Icon::CornerDownRight,
        t("tags.row_move"),
        t("tags.move_sub"),
    )
    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
        this.start_keyword_move(cx);
    }));
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(section_head(t("tags.organise")))
        .child(merge)
        .children(
            app.tag_manager
                .merge
                .is_some()
                .then(|| render_merge_picker(app, cx)),
        )
        .child(save_smart)
        .child(move_row)
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

/// An ORGANISE row: bg-0 r8 h46, icon 15, label 12 t1, sub 10 t3, chevron.
fn organise_row(selector: &'static str, icon: Icon, label: String, sub: String) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
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
        .child(render_icon(icon, px(15.0), theme::t2()))
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
}

/// Most targets the picker lists; typing narrows the rest.
const MERGE_OPTIONS_SHOWN: usize = 8;

/// The Merge into… picker: the filter field, the matching keywords (the
/// highlighted one is what ↵ merges into, a click merges too) and the hint.
fn render_merge_picker(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let options = app.tag_merge_options();
    let highlight = app.tag_merge_highlight();
    let source = app.tag_manager.focused.clone().unwrap_or_default();
    let list: AnyElement = if options.is_empty() {
        div()
            .text_size(px(10.5))
            .text_color(theme::t3())
            .child(t("tags.merge_none"))
            .into_any_element()
    } else {
        div()
            .id("tag-detail-merge-options")
            .role(Role::ListBox)
            .aria_label(t("tags.merge_targets"))
            .flex()
            .flex_col()
            // ponytail: a window of 8 around the highlight, no virtual list;
            // the filter is the way to a far keyword.
            .children(
                options
                    .into_iter()
                    .enumerate()
                    .skip((highlight + 1).saturating_sub(MERGE_OPTIONS_SHOWN))
                    .take(MERGE_OPTIONS_SHOWN)
                    .map(|(i, target)| {
                        let selector = format!("tag-detail-merge-option-{target}");
                        let source = source.clone();
                        let pick = target.clone();
                        theme::menu_item(ElementId::Name(selector.clone().into()), i == highlight)
                            .debug_selector(move || selector.clone())
                            .role(Role::ListBoxOption)
                            .aria_label(target.clone())
                            .child(div().truncate().child(target))
                            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                                this.merge_tag_into(source.clone(), pick.clone(), cx);
                            }))
                    }),
            )
            .into_any_element()
    };
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child(
            theme::field("tag-detail-merge-field")
                .debug_selector(|| "tag-detail-merge-field".to_string())
                .border_color(theme::focus_ring())
                .children(app.field_input_for(TextTarget::TagMergeFilter)),
        )
        .child(list)
        .child(
            div()
                .text_size(px(10.5))
                .text_color(theme::t3())
                .child(t("tags.merge_hint")),
        )
}
