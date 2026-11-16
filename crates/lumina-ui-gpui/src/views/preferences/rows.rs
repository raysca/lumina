//! Settings row widgets (frame `l85pQN`): a labelled section around a
//! group of rows, each row a label column with an optional description
//! and its controls on the right. Design 15 reuses these for its pages.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

/// A section: its 10/600 label (with an optional 11.5 note on the right),
/// 9 px above a group of rows.
pub(super) fn section(
    label: &'static str,
    note: Option<SharedString>,
    rows: impl IntoIterator<Item = Div>,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(9.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(theme::section_label(label))
                .children(note.map(|note| {
                    div()
                        .text_size(px(11.5))
                        .text_color(theme::t3())
                        .child(note)
                })),
        )
        .child(group(rows))
}

/// A `bg_1` card, 12 px radius, with a bottom line between its rows.
fn group(rows: impl IntoIterator<Item = Div>) -> Div {
    let mut rows: Vec<Div> = rows.into_iter().collect();
    let last = rows.pop();
    div()
        .flex()
        .flex_col()
        .px(px(16.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(theme::line())
        .bg(theme::bg_1())
        .children(
            rows.into_iter()
                .map(|row| row.border_b_1().border_color(theme::line())),
        )
        .children(last)
}

/// One row: the label column takes the free width; `controls` sit on the
/// right, 24 px away. 43.5 tall, or 62.5 with a description.
pub(super) fn row(label: &'static str, description: Option<&'static str>, controls: Div) -> Div {
    div()
        .min_h(px(43.5))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(24.0))
        .child(
            div()
                .flex_1()
                // A long path squeezes its pill, not the label.
                .min_w(px(120.0))
                .flex()
                .flex_col()
                .when(description.is_some(), |s| s.py(px(14.0)))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme::t1())
                        .child(label),
                )
                .children(description.map(|text| {
                    div()
                        .text_size(px(11.5))
                        .line_height(px(16.0))
                        .text_color(theme::t2())
                        .child(text)
                })),
        )
        .child(controls)
}

/// The controls of a row, 8 px apart. It may shrink (a path pill
/// truncates); its buttons do not.
pub(super) fn controls() -> Div {
    div()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
}

/// A read-only value beside a row's controls: mono 11.5 `t2` (counts and
/// sizes) or sans 12.5 `t1`.
pub(super) fn value(text: &str, mono: bool) -> Div {
    let v = div().min_w_0().truncate().child(text.to_string());
    if mono {
        v.font_family(theme::FONT_MONO)
            .text_size(px(11.5))
            .text_color(theme::t2())
    } else {
        v.text_size(px(12.5)).text_color(theme::t1())
    }
}

/// A path in a 34 px pill with a folder icon. It truncates from the start,
/// keeping the folder name, and its tooltip shows the full path.
pub(super) fn path_pill(id: &'static str, path: String) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .tooltip(theme::tooltip_label(path.clone()))
        .h(px(34.0))
        .min_w_0()
        .px(px(11.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(theme::line_2())
        .bg(theme::bg_0())
        .child(render_icon(Icon::Folder, px(14.0), theme::t3()).flex_shrink_0())
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis_start()
                .font_family(theme::FONT_MONO)
                .text_size(px(11.5))
                .text_color(theme::t2())
                .child(path),
        )
}

type RowAction = fn(&mut LuminaApp, &mut Context<LuminaApp>);

/// A 34 px row button (`bg_3`, `line_2` outline, 12.5/500). It never
/// shrinks or wraps.
pub(super) fn row_button(
    cx: &mut Context<LuminaApp>,
    id: &'static str,
    label: &'static str,
    action: RowAction,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(SharedString::from(label))
        .h(px(34.0))
        .px(px(13.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(theme::line_2())
        .bg(theme::bg_3())
        .hover(|s| s.bg(theme::line_2()))
        .whitespace_nowrap()
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme::t1())
        .cursor_pointer()
        .child(label)
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            action(this, cx);
        }))
}
