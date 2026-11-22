//! The Add to Album picker (⇧⌘A): a filter field, the matching albums and
//! a New Album… row, on an occluding card over a dismiss backdrop.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::text_input::TextTarget;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

/// The picker for `albums.picker`, or nothing when it is closed.
pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let Some(picker) = app.albums.picker.as_ref() else {
        return Empty.into_any_element();
    };
    let rows = app.album_picker_rows();
    let highlight = picker.highlight.min(rows.len());
    let mut list: Vec<AnyElement> = rows
        .iter()
        .enumerate()
        .map(|(index, album)| {
            let id = album.id;
            row(
                ElementId::NamedInteger("album-picker-row".into(), id as u64),
                format!("album-picker-row-{id}"),
                Icon::Image,
                album.title.clone(),
                index == highlight,
            )
            .role(Role::ListItem)
            .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                this.add_picked_to_album(id, cx);
            }))
            .into_any_element()
        })
        .collect();
    list.push(
        row(
            "album-picker-new".into(),
            "album-picker-new".to_string(),
            Icon::Plus,
            t("album.picker_new"),
            highlight == rows.len(),
        )
        .role(Role::Button)
        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
            this.picker_new_album(cx);
        }))
        .into_any_element(),
    );

    div()
        .id("album-picker-backdrop")
        .debug_selector(|| "album-picker-backdrop".to_string())
        .absolute()
        .inset_0()
        .occlude()
        .bg(theme::modal_dim())
        .flex()
        .justify_center()
        .items_start()
        .pt(px(110.0))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.close_album_picker(cx)),
        )
        .child(
            div()
                .id("album-picker")
                .debug_selector(|| "album-picker".to_string())
                .role(Role::Dialog)
                .aria_label(t("album.picker_title"))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_e, _w, cx| cx.stop_propagation())
                .w(px(420.0))
                .max_h(px(420.0))
                .bg(theme::bg_2())
                .border_1()
                .border_color(theme::line_2())
                .rounded(theme::RADIUS_LG)
                .overflow_hidden()
                .shadow_2xl()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id("album-picker-filter")
                        .debug_selector(|| "album-picker-filter".to_string())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .h(px(44.0))
                        .flex_shrink_0()
                        .px(px(14.0))
                        .border_b_1()
                        .border_color(theme::line())
                        .child(render_icon(Icon::Search, theme::ICON_SM, theme::t2()))
                        .child(
                            div()
                                .flex_1()
                                .text_size(theme::TEXT_BODY)
                                .text_color(theme::t1())
                                .children(app.field_input_for(TextTarget::AlbumPicker)),
                        )
                        .child(theme::kbd("esc")),
                )
                .child(
                    div()
                        .id("album-picker-list")
                        .flex_1()
                        .min_h(px(0.0))
                        .overflow_y_scroll()
                        .track_scroll(&picker.scroll)
                        .p(px(6.0))
                        .flex()
                        .flex_col()
                        .children(list),
                )
                .child(
                    div()
                        .h(px(30.0))
                        .flex_shrink_0()
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(theme::line())
                        .bg(theme::bg_1())
                        .text_size(theme::TEXT_SMALL)
                        .text_color(theme::t2())
                        .child(theme::kbd("↑"))
                        .child(theme::kbd("↓"))
                        .child(theme::kbd("↵").ml_2())
                        .child(t("album.picker_hint_add")),
                ),
        )
        .into_any_element()
}

/// A 30px row; the highlighted one is the ↵ target.
fn row(
    id: ElementId,
    selector: String,
    icon: Icon,
    label: String,
    highlighted: bool,
) -> Stateful<Div> {
    let fg = if highlighted {
        theme::on_accent()
    } else {
        theme::t1()
    };
    div()
        .id(id)
        .debug_selector(move || selector.clone())
        .aria_label(label.clone())
        .h(px(30.0))
        .flex_shrink_0()
        .px(px(10.0))
        .rounded(theme::RADIUS_MD)
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .map(|r| {
            if highlighted {
                r.bg(theme::accent())
            } else {
                r.hover(|s| s.bg(theme::bg_3()))
            }
        })
        .child(render_icon(icon, theme::ICON_SM, fg))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme::TEXT_BODY)
                .text_color(fg)
                .child(label),
        )
}
