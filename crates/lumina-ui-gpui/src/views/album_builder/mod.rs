mod dropdowns;
mod footer;
mod header;
mod rule_row;

use gpui::*;
use lumina_core::SmartAlbum;

use crate::theme;
use crate::{generate_gradient_image, LuminaApp};

pub struct AlbumBuilderModal;

impl AlbumBuilderModal {
    pub fn render(app: &LuminaApp, cx: &mut Context<LuminaApp>) -> impl IntoElement {
        if !app.album_draft.modal_open {
            return Empty.into_any_element();
        }

        let match_mode = app.album_draft.match_mode;
        let rules_count = app.album_draft.rules.len();

        let match_count = if let Some(ref state) = app.grid_state {
            let temp_album = SmartAlbum::new(
                app.album_draft.album_title.clone(),
                "folder".to_string(),
                match_mode,
                app.album_draft.rules.clone(),
            );
            state.engine.query_smart_album_count(&temp_album)
        } else {
            24
        };

        // Neutral placeholder tiles for the live preview strip; real matches are
        // shown after the album is created.
        let placeholder_thumb = generate_gradient_image((60, 60, 65), (35, 35, 40), 100, 100);
        let thumb1 = placeholder_thumb.clone();
        let thumb2 = placeholder_thumb.clone();
        let thumb3 = placeholder_thumb;

        div()
            .id("album-builder-backdrop")
            .debug_selector(|| "album-builder-backdrop".to_string())
            .absolute()
            .inset_0()
            .occlude()
            .bg(rgba(0x09090b99))
            .flex()
            .justify_center()
            .items_center()
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
                    .w(px(640.0))
                    .max_h(px(620.0))
                    .bg(theme::bg_app())
                    .border_1()
                    .border_color(theme::border())
                    .rounded_xl()
                    .shadow_lg()
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .on_mouse_down(MouseButton::Left, |_e, _w, cx| {
                        cx.stop_propagation();
                    })
                    .child(header::render_title_row(app, cx))
                    .child(header::render_presets_row(cx))
                    .child(header::render_match_mode_row(match_mode, cx))
                    .child(
                        div()
                            .id("album-builder-scroll-content")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap_2p5()
                            .children(app.album_draft.rules.iter().enumerate().map(
                                |(idx, rule)| {
                                    rule_row::render_rule_row(app, idx, rule, rules_count, cx)
                                },
                            )),
                    )
                    .child(footer::render_live_preview(
                        match_count,
                        thumb1,
                        thumb2,
                        thumb3,
                    ))
                    .child(footer::render_footer_actions(app, cx)),
            )
            .into_any_element()
    }
}
