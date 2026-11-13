//! Capture rows for the docked inspector: the camera facts a file has, in
//! the Detail design order.

use gpui::*;
use lumina_core::MediaItem;

use crate::app::smart_draft::camera_name;
use crate::i18n::{t, t_with};
use crate::theme;

/// One Capture row: its localised key, automation name and value.
pub(crate) struct CaptureRow {
    pub key: String,
    pub selector: &'static str,
    pub value: String,
}

/// A text EXIF field with something in it: the scanner stores `Some("")`
/// for an empty tag.
fn filled(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

/// The capture facts the file has, in design order. Unknown or blank values
/// are left out, so an item with no EXIF gets no rows.
pub(crate) fn capture_rows(item: &MediaItem, focal_35mm: Option<u32>) -> Vec<CaptureRow> {
    let camera = camera_name(
        item.make.as_deref().unwrap_or_default(),
        item.model.as_deref().unwrap_or_default(),
    );
    let exposure = [
        item.f_number.map(|f| format!("ƒ/{f:.1}")),
        filled(&item.exposure_time),
        item.iso.map(|iso| format!("ISO {iso}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let focal = item.focal_length.map(|mm| {
        let actual = format!("{mm:.0} mm");
        // A full-frame "50 mm · 50 eq." says nothing new.
        match focal_35mm.filter(|eq| *eq != mm.round() as u32) {
            Some(eq) => format!(
                "{actual} · {}",
                t_with("inspector.focal_equivalent", &[&eq.to_string()])
            ),
            None => actual,
        }
    });

    [
        ("inspector.camera", "inspector-capture-camera", camera),
        (
            "inspector.lens",
            "inspector-capture-lens",
            filled(&item.lens),
        ),
        (
            "inspector.exposure",
            "inspector-capture-exposure",
            (!exposure.is_empty()).then_some(exposure),
        ),
        ("inspector.focal_length", "inspector-capture-focal", focal),
    ]
    .into_iter()
    .filter_map(|(key, selector, value)| {
        value.map(|value| CaptureRow {
            key: t(key),
            selector,
            value,
        })
    })
    .collect()
}

/// Capture row (Detail design): 19px, key Inter 11.5 `t3`, value mono 11
/// `t1` right-aligned. A long value (lens names) truncates; its tooltip
/// holds the full text.
pub(crate) fn render_capture_row(row: CaptureRow) -> Stateful<Div> {
    let selector = row.selector;
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .h(px(19.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap_2()
        .tooltip(theme::tooltip_label(row.value.clone()))
        .child(
            div()
                .flex_none()
                .text_size(px(11.5))
                .text_color(theme::t3())
                .child(row.key),
        )
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(11.0))
                .font_family(theme::FONT_MONO)
                .text_color(theme::t1())
                .text_right()
                .child(row.value),
        )
}
