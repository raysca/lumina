use gpui::*;
use lumina_core::MediaItem;

use crate::theme;
use crate::views::inspector::body::render_meta_row;

pub fn render_camera_tab(item: &MediaItem, type_label: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_lg()
        .bg(theme::bg_pane())
        .border_1()
        .border_color(theme::border_subtle())
        .child(theme::section_label("CAMERA & OPTICS"))
        // Progressive disclosure: hide if unknown!
        .children((item.model.is_some() || item.make.is_some()).then(|| {
            let camera_name = item.model.as_deref().or(item.make.as_deref()).unwrap_or("");
            render_meta_row("Camera", camera_name)
        }))
        .children(item.lens.as_ref().map(|lens| render_meta_row("Lens", lens)))
        .children(
            item.focal_length
                .map(|fl| render_meta_row("Focal Length", &format!("{:.0}mm", fl))),
        )
        .children(
            item.f_number
                .map(|f| render_meta_row("Aperture", &format!("ƒ/{:.1}", f))),
        )
        .children(
            item.exposure_time
                .as_ref()
                .map(|exp| render_meta_row("Shutter Speed", exp)),
        )
        .children(item.iso.map(|iso| render_meta_row("ISO", &iso.to_string())))
        .children((item.width > 0 && item.height > 0).then(|| {
            let mp = (item.width as f64 * item.height as f64) / 1_000_000.0;
            render_meta_row(
                "Resolution",
                &format!("{} × {} ({:.1} MP)", item.width, item.height, mp),
            )
        }))
        .child(render_meta_row("Format", type_label))
        .child(render_meta_row(
            "Captured",
            &format!("{:04}-{:02}-{:02}", item.year, item.month, item.day),
        ))
        .into_any_element()
}
