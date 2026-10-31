use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaItem;

use crate::app::info_details::InfoDetails;
use crate::theme;
use crate::views::inspector::tabs::info::render_info_label_value_row;

/// Camera tab (v3): ƒ · shutter · ISO · focal strip, then one row per fact the
/// file actually has.
pub fn render_camera_tab(item: &MediaItem, details: Option<&InfoDetails>) -> AnyElement {
    let file = details.map(|d| &d.file);
    let aperture = item.f_number.map(|f| format!("ƒ/{f:.1}"));
    let shutter = item.exposure_time.as_ref().map(|t| format!("{t} s"));
    let iso = item.iso.map(|iso| format!("ISO {iso}"));
    let focal = item.focal_length.map(|mm| format!("{mm:.0} mm"));
    let focal_row = focal
        .as_ref()
        .map(|focal| match file.and_then(|f| f.focal_length_35mm) {
            Some(equiv) => format!("{focal} ({equiv} mm equivalent)"),
            None => focal.clone(),
        });
    let exposure = [&shutter, &aperture, &iso]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");

    let rows: Vec<(&str, Option<String>)> = vec![
        ("Make", item.make.clone()),
        ("Model", item.model.clone()),
        ("Lens", item.lens.clone()),
        ("Focal length", focal_row),
        ("Exposure", (!exposure.is_empty()).then_some(exposure)),
        (
            "Exposure bias",
            file.and_then(|f| f.exposure_bias_ev).map(format_ev),
        ),
        ("Program", file.and_then(|f| f.exposure_program.clone())),
        ("Metering", file.and_then(|f| f.metering_mode.clone())),
        ("Flash", file.and_then(|f| f.flash.clone())),
        ("White balance", file.and_then(|f| f.white_balance.clone())),
        ("Software", file.and_then(|f| f.software.clone())),
    ];
    let has_any = rows.iter().any(|(_, v)| v.is_some());

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .id("camera-summary-strip")
                .debug_selector(|| "camera-summary-strip".to_string())
                .w_full()
                .flex()
                .flex_row()
                .rounded(theme::RADIUS_MD)
                .border_1()
                .border_color(theme::border_subtle())
                .bg(theme::bg_app())
                .children(
                    [
                        (aperture, "Aperture"),
                        (item.exposure_time.clone(), "Shutter"),
                        (item.iso.map(|i| i.to_string()), "ISO"),
                        (focal, "Focal length"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (value, label))| {
                        div()
                            .flex_1()
                            .py_2()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_0p5()
                            .when(i > 0, |cell| {
                                cell.border_l_1().border_color(theme::border_subtle())
                            })
                            .child(
                                div()
                                    .text_size(px(15.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::text())
                                    .child(value.unwrap_or_else(|| "—".to_string())),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::text_label())
                                    .child(label),
                            )
                    }),
                ),
        )
        .child(if has_any {
            div()
                .w_full()
                .flex()
                .flex_col()
                .children(rows.into_iter().filter_map(|(label, value)| {
                    value.map(|value| render_info_label_value_row(label, &value))
                }))
                .into_any_element()
        } else {
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("This file has no camera information.")
                .into_any_element()
        })
        .into_any_element()
}

/// −0.7 EV, +1 EV, 0 EV.
fn format_ev(ev: f64) -> String {
    let rounded = (ev * 10.0).round() / 10.0;
    if rounded == 0.0 {
        "0 EV".to_string()
    } else if rounded.fract() == 0.0 {
        format!(
            "{}{} EV",
            if rounded > 0.0 { "+" } else { "−" },
            rounded.abs() as i64
        )
    } else {
        format!(
            "{}{:.1} EV",
            if rounded > 0.0 { "+" } else { "−" },
            rounded.abs()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::format_ev;

    #[core::prelude::v1::test]
    fn exposure_bias_reads_like_a_camera() {
        assert_eq!(format_ev(-2.0 / 3.0), "−0.7 EV");
        assert_eq!(format_ev(1.0), "+1 EV");
        assert_eq!(format_ev(0.01), "0 EV");
    }
}
