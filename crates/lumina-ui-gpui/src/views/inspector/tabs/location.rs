use gpui::*;
use lumina_core::MediaItem;

use crate::theme;
use crate::views::inspector::body::render_meta_row;
use crate::LuminaApp;

pub fn render_location_tab(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    input_active: bool,
    input_buffer: &str,
) -> AnyElement {
    let has_coords = item.latitude.is_some() && item.longitude.is_some();
    let coords_text = if let (Some(lat), Some(lon)) = (item.latitude, item.longitude) {
        format!("{lat:.5}, {lon:.5}")
    } else {
        "Add latitude, longitude".to_string()
    };
    let app_location_start = weak_app;

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_lg()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border_subtle())
        .child(theme::section_label("GEOGRAPHY & GPS"))
        .children(item.city.as_ref().map(|c| render_meta_row("City", c)))
        .children(
            item.admin1_region
                .as_ref()
                .map(|r| render_meta_row("Region", r)),
        )
        .children(
            item.country_code
                .as_ref()
                .map(|c| render_meta_row("Country", c)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("Coordinates"),
                )
                .child(
                    div()
                        .id("inspector-location-box")
                        .debug_selector(|| "inspector-location-box".to_string())
                        .w_full()
                        .px_2p5()
                        .py_1p5()
                        .bg(theme::bg_app())
                        .border_1()
                        .border_color(if input_active {
                            theme::accent()
                        } else {
                            theme::border_subtle()
                        })
                        .hover(|s| {
                            if input_active {
                                s
                            } else {
                                s.border_color(theme::border_strong())
                            }
                        })
                        .rounded_md()
                        .cursor_text()
                        .text_xs()
                        .font_family(theme::FONT_MONO)
                        .text_color(if has_coords || input_active {
                            theme::text()
                        } else {
                            theme::text_subtle()
                        })
                        .child(if input_active {
                            if input_buffer.is_empty() {
                                "latitude, longitude|".to_string()
                            } else {
                                format!("{}|", input_buffer)
                            }
                        } else {
                            coords_text
                        })
                        .on_click(move |_e, _w, cx| {
                            if !input_active {
                                if let Some(app) = app_location_start.upgrade() {
                                    app.update(cx, |this, cx| {
                                        this.start_location_input(cx);
                                    });
                                }
                            }
                        }),
                ),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("Decimal degrees; leave empty to clear the location."),
        )
        .into_any_element()
}
