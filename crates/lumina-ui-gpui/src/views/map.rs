//! The offline map on Places: the bundled land outline in an
//! equirectangular projection, with geotagged items as clusters. State and
//! keys are in `app/map.rs`. The land is decorative; the clusters are
//! buttons.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::Cluster;

use crate::app::map::{place, Projection, BASEMAP};
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;

/// Circle size for a count: wider for more digits.
fn diameter(count: usize) -> f32 {
    16.0 + 7.0 * count.to_string().len() as f32
}

/// The widest circle (a seven-digit count), for the visible-band padding.
const MAX_DIAMETER: f32 = 16.0 + 7.0 * 7.0;

/// A centred message over the map (status role).
fn notice(id: &'static str, title: String, body: String) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Status)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::text())
                .child(title),
        )
        .child(div().text_xs().text_color(theme::text_subtle()).child(body))
}

fn cluster_label(c: &Cluster) -> String {
    let key = if c.count == 1 {
        "map.cluster_one"
    } else {
        "map.cluster_other"
    };
    t(key)
        .replace("{n}", &c.count.to_string())
        .replace("{lat}", &format!("{:.1}", c.lat))
        .replace("{lon}", &format!("{:.1}", c.lon))
}

fn pin(index: usize, c: &Cluster, focused: bool, app: Entity<LuminaApp>) -> AnyElement {
    let count = c.count;
    div()
        .id(("map-cluster", index))
        .debug_selector(move || format!("map-cluster-{index}"))
        .role(Role::Button)
        .aria_label(cluster_label(c))
        .aria_keyshortcuts("Enter")
        .relative()
        .size(px(diameter(count)))
        .rounded_full()
        .bg(theme::accent())
        .hover(|s| s.bg(theme::accent_hover()))
        .border_2()
        .border_color(if focused {
            theme::focus_ring()
        } else {
            theme::accent_fg()
        })
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            div()
                .debug_selector(move || format!("map-cluster-label-{index}-{count}"))
                .text_xs()
                .font_family(theme::FONT_MONO)
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme::accent_fg())
                .child(count.to_string()),
        )
        .when(focused, |pin| {
            pin.child(
                div()
                    .debug_selector(|| "map-cluster-focused".to_string())
                    .absolute()
                    .inset_0(),
            )
        })
        .on_click(move |_, _, cx| {
            app.update(cx, |this, cx| {
                if !this.map.dragged {
                    this.apply_map_cluster(index, cx);
                }
            });
        })
        .into_any_element()
}

fn zoom_button(
    id: &'static str,
    icon: Icon,
    label: String,
    step: i8,
    cx: &mut Context<LuminaApp>,
) -> impl IntoElement {
    theme::control(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
        .bg(theme::bg_control())
        .border_color(theme::border())
        .child(render_icon(icon, px(14.0), theme::text()))
        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
            this.zoom_map(step, cx);
        }))
}

/// The map, filling the grid's place.
pub fn render(this: &LuminaApp, cx: &mut Context<LuminaApp>) -> AnyElement {
    let (zoom, center, focus) = (this.map.zoom, this.map.center, this.map.focus);
    let clusters = this.map.clusters.clone();
    let viewport = this.map.viewport.clone();
    let empty = this.map.points.as_ref().is_some_and(|p| p.is_empty());
    let failed = this.map.failed && this.map.points.is_none();
    let dragging = this.map.drag.is_some();
    let app = cx.entity();
    let drag_app = app.clone();

    // Clusters are laid out here, where the map's size is known, so they
    // follow a resize in the same frame. Only those on the map are visited:
    // they are sorted by longitude, so the visible band is found by binary
    // search and only its clusters are tested and built.
    let layer = canvas(
        move |bounds, window, cx| {
            viewport.set(bounds.size);
            let projection = Projection::new(bounds, zoom, center);
            let pad = MAX_DIAMETER / 2.0 / projection.scale;
            let west = projection.to_degrees(bounds.origin).0 - pad;
            let east = projection.to_degrees(bounds.bottom_right()).0 + pad;
            let first = clusters.partition_point(|c| c.lon < west);
            let last = clusters.partition_point(|c| c.lon <= east).max(first);
            let mut pins = Vec::new();
            for (index, c) in clusters.iter().enumerate().take(last).skip(first) {
                let at = projection.to_screen(c.lon, c.lat);
                let r = px(diameter(c.count) / 2.0);
                let visible = at.x + r >= bounds.left()
                    && at.x - r <= bounds.right()
                    && at.y + r >= bounds.top()
                    && at.y - r <= bounds.bottom();
                if !visible {
                    continue;
                }
                let mut element = pin(index, c, focus == Some(index), app.clone());
                element.prepaint_as_root(
                    point(at.x - r, at.y - r),
                    size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                    window,
                    cx,
                );
                pins.push(element);
            }
            (projection, pins)
        },
        move |bounds, (projection, mut pins), window, cx| {
            window.paint_quad(fill(bounds, theme::bg_viewer()));
            if let Some(basemap) = BASEMAP.as_ref() {
                let offset = projection.basemap_offset();
                window.paint_path(
                    place(&basemap.land, projection.scale, offset),
                    theme::bg_control_hover(),
                );
                if let Some(holes) = &basemap.holes {
                    window.paint_path(place(holes, projection.scale, offset), theme::bg_viewer());
                }
            }
            for pin in &mut pins {
                pin.paint(window, cx);
            }
            // While a press lasts its move and release are followed at
            // window level, so leaving the map neither stops nor jumps it.
            if dragging {
                let mover = drag_app.clone();
                window.on_mouse_event(move |e: &MouseMoveEvent, phase, _w, cx| {
                    if phase == DispatchPhase::Bubble {
                        let held = e.pressed_button == Some(MouseButton::Left);
                        mover.update(cx, |this, cx| this.drag_map(e.position, held, cx));
                    }
                });
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _w, cx| {
                    if phase == DispatchPhase::Bubble {
                        drag_app.update(cx, |this, cx| this.end_map_drag(cx));
                    }
                });
            }
        },
    )
    .absolute()
    .inset_0();

    div()
        .id("map-view")
        .debug_selector(|| "map-view".to_string())
        .role(Role::Group)
        .aria_label(t("map.label"))
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .overflow_hidden()
        .bg(theme::bg_viewer())
        .child(layer)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, _w, cx| this.begin_map_drag(e.position, cx)),
        )
        .when(failed, |map| {
            map.child(notice(
                "map-error",
                t("map.error_title"),
                t("map.error_body"),
            ))
        })
        .when(empty, |map| {
            map.child(notice(
                "map-empty",
                t("map.empty_title"),
                t("map.empty_body"),
            ))
        })
        .child(
            div()
                .absolute()
                .top_3()
                .right_3()
                .flex()
                .flex_col()
                .gap_1()
                .child(zoom_button(
                    "map-zoom-in",
                    Icon::Plus,
                    t("map.zoom_in"),
                    1,
                    cx,
                ))
                .child(zoom_button(
                    "map-zoom-out",
                    Icon::Minus,
                    t("map.zoom_out"),
                    -1,
                    cx,
                )),
        )
        .child(
            div()
                .absolute()
                .bottom_2()
                .left_3()
                .text_xs()
                .text_color(theme::text_subtle())
                .child(t("map.hint")),
        )
        .into_any_element()
}
