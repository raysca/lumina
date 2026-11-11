//! Location section of the docked inspector (Detail design): an offline
//! mini map, the bundled land outline around the item with its pin, and
//! "Open map" to Places. No network: no streets, only the coastline.

use std::rc::Rc;

use gpui::*;
use lumina_core::MediaItem;

use crate::app::map::{place, Projection, BASEMAP};
use crate::i18n::{t, t_with};
use crate::theme;
use crate::views::inspector::summary::{place_line, section_title};
use crate::views::inspector::tabs::location::format_coordinates;
use crate::LuminaApp;

/// The section, or `None` without coordinates (no "—" placeholder).
pub(crate) fn render_location_section(
    weak_app: WeakEntity<LuminaApp>,
    item: &MediaItem,
) -> Option<Div> {
    let (lat, lon) = item.latitude.zip(item.longitude)?;
    let coords = format_coordinates(lat, lon);
    let label = match place_line(item) {
        Some(place) => format!("{place} · {coords}"),
        None => coords,
    };
    let open = div()
        .id("inspector-open-map")
        .debug_selector(|| "inspector-open-map".to_string())
        .role(Role::Button)
        .aria_label(t("inspector.open_map"))
        .tooltip(theme::tooltip_label(t("inspector.open_map_tooltip")))
        .text_size(px(11.0))
        .text_color(theme::accent())
        .cursor_pointer()
        .child(t("inspector.open_map"))
        .on_click(move |_e, _w, cx| {
            if let Some(app) = weak_app.upgrade() {
                app.update(cx, |this, cx| this.show_item_on_map(cx));
            }
        });
    Some(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(section_title("inspector.location"))
                    .child(div().flex_1())
                    .child(open),
            )
            .child(render_card((lon as f32, lat as f32), label)),
    )
}

/// Card h 132, radius 6, outline `line`: land in `bg_3` on `bg_stage`
/// water, the pin (12 `accent` with a 34 `accent_soft` halo) and the label
/// pill bottom-left.
// ponytail: the land is clipped to the card's rectangle, not its 6 px
// corners; GPUI content masks are rectangular.
fn render_card(center: (f32, f32), label: String) -> Stateful<Div> {
    let map = canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let projection = Projection::new(bounds, CARD_ZOOM, center);
            if let Some(crop) = land_crop(center.0, center.1) {
                let offset = projection.basemap_offset();
                window.paint_path(place(&crop.land, projection.scale, offset), theme::bg_3());
                if let Some(holes) = &crop.holes {
                    window.paint_path(place(holes, projection.scale, offset), theme::bg_stage());
                }
            }
            // Near a pole or the date line the centre is clamped, so the
            // pin is drawn where the item is, not at the middle.
            let at = projection.to_screen(center.0, center.1);
            let circle = |d: f32, bg: Rgba, border: f32| {
                let r = px(d / 2.0);
                quad(
                    Bounds::new(point(at.x - r, at.y - r), size(r * 2.0, r * 2.0)),
                    r,
                    bg,
                    px(border),
                    theme::accent(),
                    BorderStyle::Solid,
                )
            };
            window.paint_quad(circle(34.0, theme::accent_soft(), 1.0));
            window.paint_quad(circle(12.0, theme::accent(), 0.0));
        },
    )
    .absolute()
    .inset_0();

    div()
        .id("inspector-location-map")
        .debug_selector(|| "inspector-location-map".to_string())
        .role(Role::Image)
        .aria_label(t_with("inspector.location_map_label", &[&label]))
        .relative()
        .w_full()
        .h(px(132.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(theme::line())
        .bg(theme::bg_stage())
        .overflow_hidden()
        .child(map)
        // A long place truncates inside the card.
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .right(px(8.0))
                .bottom(px(8.0))
                .flex()
                .child(
                    div()
                        .debug_selector(|| "inspector-location-label".to_string())
                        .min_w_0()
                        .h(px(20.0))
                        .px(px(6.0))
                        .rounded(px(4.0))
                        .bg(theme::media_scrim())
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(9.5))
                        .font_family(theme::FONT_MONO)
                        .text_color(theme::on_media())
                        .child(label),
                ),
        )
}

/// Half the land crop around a location, in degrees (lon, lat). Wide
/// enough for the inspector's card at [`CARD_ZOOM`] in a panel up to about
/// 1400 px wide, with the centre clamped near a pole or the date line.
const CROP_HALF: (f32, f32) = (60.0, 25.0);

/// The basemap's triangles that touch `CROP_HALF` around a location, still
/// in degree space: the inspector card places a few hundred vertices per
/// frame instead of the whole world.
struct LandCrop {
    land: Path<Pixels>,
    holes: Option<Path<Pixels>>,
}

fn crop(path: &Path<Pixels>, (lon, lat): (f32, f32)) -> Path<Pixels> {
    let (x, y) = (lon + 180.0, 90.0 - lat);
    let mut out = Path::new(point(px(x), px(y)));
    for tri in path.vertices.as_chunks::<3>().0 {
        let xs = tri.iter().map(|v| f32::from(v.xy_position.x));
        let ys = tri.iter().map(|v| f32::from(v.xy_position.y));
        let (x0, x1) = xs.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
        let (y0, y1) = ys.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
        if x1 >= x - CROP_HALF.0
            && x0 <= x + CROP_HALF.0
            && y1 >= y - CROP_HALF.1
            && y0 <= y + CROP_HALF.1
        {
            out.push_triangle(
                (tri[0].xy_position, tri[1].xy_position, tri[2].xy_position),
                (tri[0].st_position, tri[1].st_position, tri[2].st_position),
            );
        }
    }
    out
}

/// The cached crop and its location's (lon, lat) bits.
type CropCache = Option<((u32, u32), Rc<LandCrop>)>;

thread_local! {
    // ponytail: one entry, the item on show; the panel shows one item.
    static LAND_CROP: std::cell::RefCell<CropCache> =
        const { std::cell::RefCell::new(None) };
}

/// The land around (lon, lat), cut from [`BASEMAP`] once per location and
/// cached, so a frame of the inspector card does no scan of the world.
fn land_crop(lon: f32, lat: f32) -> Option<Rc<LandCrop>> {
    let key = (lon.to_bits(), lat.to_bits());
    LAND_CROP.with(|cache| {
        if let Some((k, crop)) = cache.borrow().as_ref() {
            if *k == key {
                return Some(crop.clone());
            }
        }
        let basemap = BASEMAP.as_ref()?;
        let cut = Rc::new(LandCrop {
            land: crop(&basemap.land, (lon, lat)),
            holes: basemap.holes.as_ref().map(|h| crop(h, (lon, lat))),
        });
        *cache.borrow_mut() = Some((key, cut.clone()));
        Some(cut)
    })
}

/// Zoom of the inspector's location card: about 25° of longitude, the
/// same zoom Open map lands on, so the map continues what the card showed.
const CARD_ZOOM: u8 = crate::app::map::FOCUS_ZOOM;

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn land_crop_is_a_cached_part_of_the_world() {
        let world = BASEMAP.as_ref().expect("the basemap tessellates");
        let lisbon = land_crop(-9.14, 38.71).expect("a crop of the basemap");
        let n = lisbon.land.vertices.len();
        assert!(n > 0, "Lisbon has land around it");
        assert!(
            n < world.land.vertices.len() / 2,
            "the crop keeps only nearby triangles: {n} of {}",
            world.land.vertices.len()
        );
        assert_eq!(n % 3, 0, "the crop is whole triangles");
        assert!(
            Rc::ptr_eq(&lisbon, &land_crop(-9.14, 38.71).unwrap()),
            "the same location reuses the cached crop"
        );
        assert!(
            !Rc::ptr_eq(&lisbon, &land_crop(135.76, 35.01).unwrap()),
            "a new location cuts a new crop"
        );
    }
}
