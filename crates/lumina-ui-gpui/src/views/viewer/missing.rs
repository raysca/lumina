//! Original offline (Design 10): the warn bar over the stage and the
//! "Original not available" card for an item whose file is gone.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::render::empty_state::{empty_body, empty_button, empty_headline};
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

/// The bar and, unless "Keep preview" hid it, the card. Empty for an item
/// whose file is there. The cached image, when there is one, stays on the
/// stage behind them.
pub(super) fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> Vec<AnyElement> {
    let Some(item) = this
        .selected_item
        .as_ref()
        .filter(|item| this.missing_items.contains(&item.id))
    else {
        return Vec::new();
    };
    // Only promise a cached preview when one is on screen.
    let preview = this.viewer_image.is_some();
    let drive = this.missing_drive(item.id).map(str::to_string);
    let mut elements = vec![bar(drive.as_deref(), preview)];
    if this.missing_card_shown() {
        elements.push(card(this, drive.is_some(), preview, cx));
    }
    elements
}

fn bar(drive: Option<&str>, preview: bool) -> AnyElement {
    let line = |text: SharedString| div().flex_none().child(text);
    let (sentence, words) = match drive {
        Some(name) => {
            let template = t(if preview {
                "viewer.drive_preview"
            } else {
                "viewer.drive_no_preview"
            });
            let (before, after) = template.split_once("{name}").unwrap_or((&template, ""));
            let name = SharedString::from(name.to_string());
            let words = vec![
                line(before.to_string().into()),
                // A long drive name loses its middle; the sentence stays.
                div()
                    .debug_selector(|| "viewer-missing-drive".to_string())
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis_middle()
                    .font_family(theme::FONT_MONO)
                    .children(text_marker("viewer-missing-drive", &name))
                    .child(name.clone()),
                line(after.to_string().into()),
            ];
            (format!("{before}{name}{after}"), words)
        }
        None => {
            let sentence = t(if preview {
                "viewer.missing_preview"
            } else {
                "viewer.missing_no_preview"
            });
            let words = vec![div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .child(sentence.clone())];
            (sentence, words)
        }
    };
    let sentence = SharedString::from(sentence);
    div()
        .id("viewer-missing-bar")
        .debug_selector(|| "viewer-missing-bar".to_string())
        .role(Role::Alert)
        .aria_label(sentence.clone())
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(39.5))
        .px(px(16.0))
        .bg(theme::warn_surface())
        .border_b_1()
        .border_color(theme::warn_border())
        .flex()
        .items_center()
        .gap(px(9.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(px(12.0))
        .text_color(theme::on_warn())
        .children(text_marker("viewer-missing-bar", &sentence))
        .child(render_icon(Icon::TriangleAlert, px(14.0), theme::warn()).flex_none())
        .child(div().min_w_0().flex().children(words))
        .into_any_element()
}

fn card(this: &LuminaApp, drive: bool, preview: bool, cx: &mut Context<LuminaApp>) -> AnyElement {
    let focus = this.missing_card.focus;
    let focused = |button: Stateful<Div>, index| {
        button.when(focus == Some(index), |b| {
            b.border_color(theme::focus_ring())
        })
    };
    let locate = focused(
        empty_button(
            "viewer-missing-locate",
            true,
            t("viewer.missing_locate").into(),
            Some(Icon::Folder),
            px(32.0),
        ),
        0,
    )
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.locate_missing_selected(cx)));
    // With no cached image there is no preview to keep.
    let keep_label = t(if preview {
        "viewer.missing_keep"
    } else {
        "viewer.missing_dismiss"
    });
    let keep = focused(
        empty_button(
            "viewer-missing-keep",
            false,
            keep_label.into(),
            Some(Icon::Check),
            px(32.0),
        ),
        1,
    )
    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.keep_missing_preview(cx)));
    let title = t("viewer.missing_title");
    let body = t(if drive {
        "viewer.missing_body_drive"
    } else {
        "viewer.missing_body"
    });
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("viewer-missing-card")
                .debug_selector(|| "viewer-missing-card".to_string())
                .role(Role::Group)
                .aria_label(title.clone())
                // Clicks on the card must not reach the photo (double-click zoom).
                .occlude()
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, e: &MouseDownEvent, _, cx| {
                        super::viewport::open_viewer_menu(this, e.position, cx)
                    }),
                )
                .w(px(420.0))
                .max_w_full()
                .p(px(24.0))
                .rounded(theme::RADIUS_LG)
                .bg(theme::bg_float())
                .border_1()
                .border_color(theme::line_2())
                .shadow_xl()
                .flex()
                .flex_col()
                .items_center()
                .child(empty_headline("viewer-missing-title", title.into()).mt_0())
                .child(empty_body("viewer-missing-body", body.into()))
                .child(
                    div()
                        .mt(px(16.0))
                        .flex()
                        .flex_row()
                        .gap(px(9.0))
                        .child(locate)
                        .child(keep),
                ),
        )
        .into_any_element()
}
