//! Settings › About: version, licence and acknowledgements.

use gpui::*;

use super::button;
use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;

pub(super) fn render_about(cx: &mut Context<LuminaApp>) -> AnyElement {
    div()
        .pt(px(30.0))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(72.0))
                .rounded(px(16.0))
                .bg(theme::accent())
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(Icon::Sparkles, px(38.0), theme::on_accent())),
        )
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::t1())
                .child("Lumina"),
        )
        .child(
            div()
                .debug_selector(|| "preferences-version".to_string())
                .font_family(theme::FONT_MONO)
                .text_xs()
                .text_color(theme::t2())
                .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
        )
        .child(
            div()
                .max_w(px(380.0))
                .text_sm()
                .text_center()
                .text_color(theme::t2())
                .child(
                    "A local-first photo and video manager. No account, no network, \
                     no telemetry. Your files stay where they are.",
                ),
        )
        .child(
            div()
                .pt(px(6.0))
                .flex()
                .flex_row()
                .gap_2()
                .child(button(
                    cx,
                    "preferences-license",
                    "MIT License",
                    |this, cx| this.open_bundled_doc("LICENSE", cx),
                ))
                .child(button(
                    cx,
                    "preferences-acknowledgements",
                    "Acknowledgements",
                    |this, cx| this.open_bundled_doc("THIRD_PARTY_NOTICES.md", cx),
                )),
        )
        .into_any_element()
}
