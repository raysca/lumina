//! Capture rows for the docked inspector: the camera facts a file has, in
//! the Detail design order.

use lumina_core::{MediaItem, MediaType};

use crate::app::types::format_duration_badge;
use crate::i18n::{t, t_with};
use crate::views::inspector::summary::resolution_label;

/// One Capture row: its localised key, automation name and value.
pub(crate) struct CaptureRow {
    pub key: String,
    pub selector: &'static str,
    pub value: String,
}

/// The capture facts the file has, in design order. Unknown values are left
/// out, so an item with no EXIF gets no rows.
pub(crate) fn capture_rows(item: &MediaItem, focal_35mm: Option<u32>) -> Vec<CaptureRow> {
    // Many makers repeat the make in the model ("Canon" + "Canon EOS R5").
    let camera = match (&item.make, &item.model) {
        (Some(make), Some(model)) if model.to_lowercase().starts_with(&make.to_lowercase()) => {
            Some(model.clone())
        }
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (make, model) => make.clone().or_else(|| model.clone()),
    };
    let exposure = [
        item.f_number.map(|f| format!("ƒ/{f:.1}")),
        item.exposure_time.clone(),
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
    let video = item.media_type == MediaType::Video;
    let duration = video
        .then(|| format_duration_badge(item.duration_secs()))
        .filter(|d| !d.is_empty());
    let resolution = (video && item.width > 0 && item.height > 0)
        .then(|| resolution_label(item.width, item.height));

    [
        ("inspector.camera", "inspector-capture-camera", camera),
        (
            "inspector.lens",
            "inspector-capture-lens",
            item.lens.clone(),
        ),
        (
            "inspector.exposure",
            "inspector-capture-exposure",
            (!exposure.is_empty()).then_some(exposure),
        ),
        ("inspector.focal_length", "inspector-capture-focal", focal),
        ("inspector.duration", "inspector-capture-duration", duration),
        (
            "inspector.resolution",
            "inspector-capture-resolution",
            resolution,
        ),
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
