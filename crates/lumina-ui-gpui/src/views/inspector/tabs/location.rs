use gpui::*;
use lumina_core::MediaItem;

use crate::theme;
use crate::views::inspector::body::render_meta_row;
use crate::LuminaApp;

pub fn render_location_tab(_weak_app: WeakEntity<LuminaApp>, item: &MediaItem) -> AnyElement {
    let has_coords = item.latitude.is_some() && item.longitude.is_some();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_lg()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border_subtle())
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme::text_subtle())
                .child("GEOGRAPHY & GPS"),
        )
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
        .children(has_coords.then(|| {
            let lat = item.latitude.unwrap();
            let lon = item.longitude.unwrap();
            render_meta_row("Coordinates", &format!("{:.5}°, {:.5}°", lat, lon))
        }))
        .child(if !has_coords && item.city.is_none() {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .pt_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("No geographic metadata recorded on this file."),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_subtle())
                        .child("Coordinate editing is not available yet."),
                )
                .into_any_element()
        } else {
            Empty.into_any_element()
        })
        .into_any_element()
}
