use gpui::*;
use lumina_core::MediaItem;

use crate::theme;
use crate::views::inspector::tabs::info::render_info_label_value_row;
use crate::LuminaApp;

/// Location tab (v3): Place, Coordinates, Altitude, then Edit / nearby /
/// Remove. No map: Lumina makes no network calls.
pub fn render_location_tab(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
    input_active: bool,
    input_buffer: &str,
) -> AnyElement {
    let has_coords = item.latitude.is_some() && item.longitude.is_some();
    let place = [&item.city, &item.admin1_region, &item.country_code]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let coords = item
        .latitude
        .zip(item.longitude)
        .map(|(lat, lon)| format_coordinates(lat, lon));
    let altitude = item.altitude.map(|m| format!("{m:.0} m"));
    // ponytail: "nearby" is the same city via the existing city: search; a
    // radius search needs a geo index.
    let nearby_query = item
        .city
        .as_ref()
        .filter(|city| !city.contains('"'))
        .map(|city| format!("city:\"{city}\""));

    let app_edit = weak_app.clone();
    let app_nearby = weak_app.clone();
    let app_remove = weak_app;

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(render_info_label_value_row(
                    "Place",
                    if place.is_empty() { "—" } else { &place },
                ))
                .child(render_info_label_value_row(
                    "Coordinates",
                    coords.as_deref().unwrap_or("—"),
                ))
                .children(altitude.map(|a| render_info_label_value_row("Altitude", &a))),
        )
        .children(input_active.then(|| {
            div()
                .id("inspector-location-box")
                .debug_selector(|| "inspector-location-box".to_string())
                .w_full()
                .px_2p5()
                .py_1p5()
                .bg(theme::bg_app())
                .border_1()
                .border_color(theme::focus_ring())
                .rounded(px(6.0))
                .text_xs()
                .font_family(theme::FONT_MONO)
                .text_color(theme::text())
                .child(if input_buffer.is_empty() {
                    "latitude, longitude (Enter to save, Esc to cancel)|".to_string()
                } else {
                    format!("{input_buffer}|")
                })
        }))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .child(
                    location_button("inspector-location-edit", "Edit location…", false).on_click(
                        move |_e, _w, cx| {
                            if let Some(app) = app_edit.upgrade() {
                                app.update(cx, |this, cx| this.start_location_input(cx));
                            }
                        },
                    ),
                )
                .children(nearby_query.map(|query| {
                    location_button("inspector-location-nearby", "Show nearby photos", false)
                        .on_click(move |_e, _w, cx| {
                            if let Some(app) = app_nearby.upgrade() {
                                app.update(cx, |this, cx| this.set_search_query(&query, cx));
                            }
                        })
                }))
                .children(has_coords.then(|| {
                    location_button("inspector-location-remove", "Remove location", true).on_click(
                        move |_e, _w, cx| {
                            if let Some(app) = app_remove.upgrade() {
                                app.update(cx, |this, cx| this.remove_location_selected(cx));
                            }
                        },
                    )
                })),
        )
        .into_any_element()
}

fn location_button(id: &'static str, label: &'static str, destructive: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .h(px(26.0))
        .px_2p5()
        .flex()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::border())
        .bg(theme::bg_raised())
        .hover(|s| s.bg(theme::bg_control_hover()))
        .cursor_pointer()
        .text_xs()
        .text_color(if destructive {
            theme::label_red()
        } else {
            theme::text()
        })
        .child(label)
}

/// 38.7101° N, 9.1366° W
pub(crate) fn format_coordinates(lat: f64, lon: f64) -> String {
    format!(
        "{:.4}° {}, {:.4}° {}",
        lat.abs(),
        if lat < 0.0 { "S" } else { "N" },
        lon.abs(),
        if lon < 0.0 { "W" } else { "E" }
    )
}

#[cfg(test)]
mod tests {
    use super::format_coordinates;

    #[core::prelude::v1::test]
    fn coordinates_use_hemispheres() {
        assert_eq!(
            format_coordinates(38.71012, -9.13661),
            "38.7101° N, 9.1366° W"
        );
        assert_eq!(
            format_coordinates(-33.8688, 151.2093),
            "33.8688° S, 151.2093° E"
        );
    }
}
