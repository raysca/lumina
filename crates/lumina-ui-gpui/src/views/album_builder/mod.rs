//! Smart album builder dialog, modelled on Photos' New Smart Album:
//! name, "Match all/any of the following conditions", one row per condition,
//! then a live count and preview of the first matches.

use crate::app::text_input::TextTarget;
pub(crate) mod footer;
mod rows;

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::smart_draft::{match_label, DraftFocus};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

pub struct AlbumBuilderModal;

impl AlbumBuilderModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        if !app.album_draft.modal_open {
            return Empty.into_any_element();
        }
        let draft = &app.album_draft;
        let focus = draft.focus;
        let popup_on_match = draft.popup.map(|p| p.target) == Some(DraftFocus::Match);

        div()
            .id("album-builder-backdrop")
            .debug_selector(|| "album-builder-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(theme::scrim())
            // Centred; the 40px margins cap the dialog at the window height − 80.
            .flex()
            .justify_center()
            .items_center()
            .py(px(40.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    this.close_album_builder(cx);
                }),
            )
            .child(
                div()
                    .id("album-builder-modal-card")
                    .debug_selector(|| "album-builder-card".to_string())
                    .w(px(840.0))
                    .max_h_full()
                    .bg(theme::bg_1())
                    // ponytail: the frame has no border or shadow; line-2 and
                    // shadow_lg are kept on purpose to lift the dialog off the page.
                    .border_1()
                    .border_color(theme::line_2())
                    .rounded(px(16.0))
                    .shadow_lg()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .text_color(theme::t1())
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(render_header(app, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .pt(px(18.0))
                            .px(px(26.0))
                            .pb(px(22.0))
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .text_size(theme::TEXT_BODY)
                                    .text_color(theme::t2())
                                    .child("Match")
                                    .child(
                                        rows::picker(
                                            "album-builder-match".to_string(),
                                            None,
                                            match_label(draft.match_mode),
                                            focus == DraftFocus::Match,
                                            DraftFocus::Match,
                                            cx,
                                        )
                                        .w(px(92.0))
                                        .h(px(34.0))
                                        .px(px(12.0))
                                        .rounded(px(9.0))
                                        .bg(theme::bg_0())
                                        .text_size(theme::TEXT_BODY)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme::t1()),
                                    )
                                    .child("of the following conditions:"),
                            )
                            .when(popup_on_match, |el| el.child(rows::render_popup(app, cx)))
                            // The condition card: rows 10 apart, no dividers.
                            .child(
                                div()
                                    .id("album-builder-scroll-content")
                                    .debug_selector(|| "album-builder-conditions".to_string())
                                    .flex_1()
                                    .min_h_0()
                                    .w_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&draft.scroll)
                                    .rounded(px(12.0))
                                    .bg(theme::bg_2())
                                    .p(px(14.0))
                                    .gap(px(10.0))
                                    .flex()
                                    .flex_col()
                                    .when(draft.rows.is_empty(), |el| {
                                        el.child(render_empty_conditions(focus, cx))
                                    })
                                    .children(
                                        draft
                                            .rows
                                            .iter()
                                            .enumerate()
                                            .map(|(i, row)| rows::render_row(app, i, row, cx)),
                                    ),
                            )
                            .child(footer::render_options(app, cx))
                            .child(footer::render_preview(app)),
                    )
                    .child(footer::render_footer(app, cx)),
            )
            .into_any_element()
    }
}

/// Badge, name field and ×; × closes without saving, like Cancel and Esc.
fn render_header(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    let draft = &app.album_draft;
    let focused = draft.focus == DraftFocus::Title;
    let close_label = t("smart_album.close");
    div()
        .flex_none()
        .pt(px(22.0))
        .px(px(26.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(
            div()
                .debug_selector(|| "album-builder-badge".to_string())
                .flex_none()
                .size(px(36.0))
                .rounded(px(10.0))
                .bg(theme::accent_soft())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Sparkles, px(18.0), theme::accent())),
        )
        .child(
            rows::text_field(
                "album-builder-title".to_string(),
                &draft.album_title,
                &t("smart_album.untitled"),
                focused,
                app.field_input_for(TextTarget::AlbumDraft)
                    .filter(|_| focused),
                DraftFocus::Title,
                cx,
            )
            .w(px(420.0))
            .h(px(42.0))
            .px(px(13.0))
            .rounded(px(10.0))
            .bg(theme::bg_0())
            .border_color(if focused {
                theme::accent().into()
            } else {
                transparent_black()
            })
            // Deviations from the frame: no −0.3 tracking (GPUI has no
            // letter-spacing) and the shared text input's t1 caret, not a 2×20
            // accent one.
            .font_family(theme::FONT_DISPLAY)
            .text_size(theme::TEXT_TITLE)
            .font_weight(FontWeight::SEMIBOLD),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("album-builder-close")
                .debug_selector(|| "album-builder-close".to_string())
                .role(Role::Button)
                .aria_label(close_label.clone())
                .flex_none()
                .size(px(28.0))
                .rounded(px(8.0))
                .bg(theme::bg_2())
                .hover(|s| s.bg(theme::bg_3()))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .tooltip(theme::tooltip_label(close_label))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    this.close_album_builder(cx);
                }))
                .child(render_icon(Icon::X, px(14.0), theme::t2())),
        )
}

/// "No conditions — every item matches." plus the way back to a condition.
fn render_empty_conditions(focus: DraftFocus, cx: &mut Context<LuminaApp>) -> impl IntoElement {
    div()
        .debug_selector(|| "album-builder-empty".to_string())
        .px(px(12.0))
        .py(px(10.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .text_size(theme::TEXT_BODY)
        .text_color(theme::t2())
        .child("No conditions — every item matches.")
        .child(rows::square_button(
            "album-rule-add-empty".to_string(),
            Icon::Plus,
            "Add condition",
            focus == DraftFocus::Add(0),
            |this, cx| this.add_draft_row_after(0, cx),
            cx,
        ))
}
