//! The app-wide import banner: a persistent strip under the error banner
//! while a plan is being built or files are being copied. It is deliberately
//! outside the import dialog, so navigating to the library never hides the
//! fact that a large import is still running.

use gpui::*;

use crate::app::LuminaApp;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::import_dialog::progress_bar;
use crate::views::page_header::text_marker;

impl LuminaApp {
    /// The banner for the running plan or copy, or nothing when idle.
    pub(crate) fn render_import_banner(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(banner) = self.import.banner() else {
            return Empty.into_any_element();
        };
        let cancel = banner.can_cancel.then(|| {
            div()
                .id("import-banner-cancel")
                .debug_selector(|| "import-banner-cancel".to_string())
                .role(Role::Button)
                .aria_label("Cancel import")
                .px_2()
                .py_1()
                .rounded(theme::RADIUS_SM)
                .cursor_pointer()
                .text_size(theme::TEXT_SMALL)
                .text_color(theme::t1())
                .hover(|s| s.bg(theme::bg_3()))
                .tooltip(theme::tooltip_label("Stop after the current file"))
                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                    if this.import.dialog.is_some() {
                        this.cancel_import_dialog(cx);
                    } else {
                        this.cancel_running_import(cx);
                    }
                }))
                .child("Cancel")
        });

        div()
            .id("import-banner")
            .debug_selector(|| "import-banner".to_string())
            .flex_none()
            .w_full()
            .px_4()
            .py_2()
            .bg(theme::bg_1())
            .border_b_1()
            .border_color(theme::line())
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(render_icon(Icon::Import, theme::TEXT_BODY, theme::accent()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .debug_selector(|| "import-banner-title".to_string())
                            .text_size(theme::TEXT_SMALL)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme::t1())
                            .children(text_marker(
                                "import-banner-title",
                                &banner.title.clone().into(),
                            ))
                            .child(banner.title),
                    )
                    .child(
                        div()
                            .debug_selector(|| "import-banner-detail".to_string())
                            .text_size(theme::TEXT_CAPTION)
                            .text_color(theme::t2())
                            .children(text_marker(
                                "import-banner-detail",
                                &banner.detail.clone().into(),
                            ))
                            .child(banner.detail),
                    ),
            )
            .children(
                banner
                    .fraction
                    .map(|fraction| progress_bar("import-banner-progress", fraction, 160.0)),
            )
            .children(cancel)
            .into_any_element()
    }
}
