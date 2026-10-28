mod dropdowns;
mod footer;
mod header;
mod rule_row;

use gpui::*;
use lumina_core::SmartAlbum;

use crate::theme;
use crate::{generate_gradient_image, get_sample_gradient, LuminaApp};

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

        let (t1, b1, w1, h1) = get_sample_gradient("Lisbon_Tram28_0412.ARW");
        let thumb1 = generate_gradient_image(t1, b1, w1, h1);
        let (t2, b2, w2, h2) = get_sample_gradient("Alfama_Rooftops.HEIC");
        let thumb2 = generate_gradient_image(t2, b2, w2, h2);
        let (t3, b3, w3, h3) = get_sample_gradient("Belem_Tower_Dusk.ARW");
        let thumb3 = generate_gradient_image(t3, b3, w3, h3);

        div()
            .id("album-builder-backdrop")
            .absolute()
            .inset_0()
            .bg(rgba(0x09090b99))
            .flex()
            .justify_center()
            .items_center()
            .child(
                div()
                    .id("album-builder-modal-card")
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
