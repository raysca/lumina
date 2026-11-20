//! EXIF detail rows for Get Info, kept out of the scan loop.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaDetails {
    /// File-system creation (birth) time, where the platform records it.
    pub created_epoch_ms: Option<i64>,
    pub color_space: Option<String>,
    pub exposure_bias_ev: Option<f64>,
    pub exposure_program: Option<String>,
    pub metering_mode: Option<String>,
    pub flash: Option<String>,
    pub white_balance: Option<String>,
    pub software: Option<String>,
    pub focal_length_35mm: Option<u32>,
}

/// Reads [`MediaDetails`] for `path`. Missing or unreadable fields are `None`;
/// this never fails, since Get Info just shows fewer rows. Does file IO: call
/// it off the UI thread.
pub fn read_media_details(path: &Path) -> MediaDetails {
    let mut details = MediaDetails {
        created_epoch_ms: std::fs::metadata(path)
            .and_then(|m| m.created())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64),
        ..Default::default()
    };
    let Ok(file) = File::open(path) else {
        return details;
    };
    let Ok(exif_data) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else {
        return details;
    };
    let text = |tag: exif::Tag| {
        exif_data
            .get_field(tag, exif::In::PRIMARY)
            .map(|f| sentence_case(&clean_string(&f.display_value().to_string())))
            .filter(|s| !s.is_empty())
    };
    details.color_space = text(exif::Tag::ColorSpace);
    details.exposure_program = text(exif::Tag::ExposureProgram);
    details.metering_mode = text(exif::Tag::MeteringMode);
    details.flash = text(exif::Tag::Flash);
    details.white_balance = text(exif::Tag::WhiteBalance);
    details.software = exif_data
        .get_field(exif::Tag::Software, exif::In::PRIMARY)
        .map(|f| clean_string(&f.display_value().to_string()))
        .filter(|s| !s.is_empty());
    details.exposure_bias_ev = exif_data
        .get_field(exif::Tag::ExposureBiasValue, exif::In::PRIMARY)
        .and_then(|f| match f.value {
            exif::Value::SRational(ref v) => v.first().map(|r| r.to_f64()),
            _ => parse_rational(&f.value),
        });
    details.focal_length_35mm = exif_data
        .get_field(exif::Tag::FocalLengthIn35mmFilm, exif::In::PRIMARY)
        .and_then(|f| parse_integer(&f.value))
        .filter(|&mm| mm > 0);
    details
}

/// "normal program" -> "Normal program" for display.
pub(crate) fn sentence_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(crate) fn clean_string(s: &str) -> String {
    s.trim().trim_matches('"').trim().to_string()
}

pub(crate) fn parse_rational(value: &exif::Value) -> Option<f64> {
    if let exif::Value::Rational(ref rationals) = value {
        if let Some(r) = rationals.first() {
            return Some(r.to_f64());
        }
    }
    None
}

pub(crate) fn parse_integer(value: &exif::Value) -> Option<u32> {
    match value {
        exif::Value::Short(ref v) => v.first().map(|&x| x as u32),
        exif::Value::Long(ref v) => v.first().copied(),
        _ => None,
    }
}

pub(crate) fn parse_dms(value: &exif::Value) -> Option<f64> {
    if let exif::Value::Rational(ref rationals) = value {
        if rationals.len() >= 3 {
            let deg = rationals[0].to_f64();
            let min = rationals[1].to_f64();
            let sec = rationals[2].to_f64();
            return Some(deg + min / 60.0 + sec / 3600.0);
        }
    }
    None
}
