//! XMP sidecar persistence for catalog metadata.
//!
//! Ratings, color labels, captions and tags live in SQLite, but that data is trapped in
//! Lumina unless it is also written next to the media. We emit a small RDF/XML
//! sidecar (photo.jpg.xmp) compatible with Adobe conventions for
//! xmp:Rating, xmp:Label, dc:description, dc:subject, lr:hierarchicalSubject
//! (keyword paths) and the rights fields (dc:creator, dc:rights,
//! xmpRights:UsageTerms, Iptc4xmpCore:CreatorContactInfo), plus the capture
//! date and GPS position when the user edited them (exif:DateTimeOriginal,
//! photoshop:DateCreated, exif:GPSLatitude/GPSLongitude), and can read it back.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::Rights;

mod owner;
mod parse;
mod render;

pub use parse::parse_xmp;
pub use render::render_xmp;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SidecarMetadata {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub rating: u8,
    /// 0 = none, 1..=6 = [`LABEL_NAMES`] (the catalog's `color_label`).
    pub color_label: u8,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    pub credit: Option<String>,
    pub rights: Rights,
    /// Capture date as wall time (no offset). Written only when the user
    /// edited it; file EXIF stays the source otherwise.
    pub date_taken: Option<chrono::NaiveDateTime>,
    /// The sidecar's date had no time of day (`2024-05-01`): `date_taken`
    /// is midnight, and import keeps the catalog's time instead. Only
    /// [`parse_xmp`] sets it; Lumina always writes the time.
    pub date_only: bool,
    /// (latitude, longitude) in degrees. Written only when the user edited it.
    pub location: Option<(f64, f64)>,
}

/// xmp:Label values for color labels 1..=6, the names Lightroom and Bridge use.
pub const LABEL_NAMES: [&str; 6] = ["Red", "Orange", "Yellow", "Green", "Blue", "Purple"];

impl SidecarMetadata {
    pub fn is_empty(&self) -> bool {
        self.title.as_deref().unwrap_or("").trim().is_empty()
            && self.caption.as_deref().unwrap_or("").trim().is_empty()
            && self.source_url.as_deref().unwrap_or("").trim().is_empty()
            && self.credit.as_deref().unwrap_or("").trim().is_empty()
            && self.rating == 0
            && self.color_label == 0
            && self.tags.is_empty()
            && self.rights.is_empty()
            && self.date_taken.is_none()
            && self.location.is_none()
    }
}

/// photo.jpg -> photo.jpg.xmp
pub fn sidecar_path(media_path: &Path) -> PathBuf {
    let mut path = media_path.as_os_str().to_owned();
    path.push(".xmp");
    PathBuf::from(path)
}

/// What [`write_sidecar`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidecarOutcome {
    Written(PathBuf),
    /// The metadata was empty and Lumina's own sidecar was deleted.
    Removed,
    /// The metadata was empty and there was no sidecar.
    Unchanged,
    /// A sidecar another app wrote (or one Lumina cannot read) is in the
    /// way; it was left untouched.
    SkippedForeign,
}

/// Writes the sidecar atomically. Empty metadata removes any existing sidecar.
/// A sidecar Lumina did not write is never rewritten or removed
/// ([`SidecarOutcome::SkippedForeign`]); see [`owner`].
pub fn write_sidecar(media_path: &Path, metadata: &SidecarMetadata) -> Result<SidecarOutcome> {
    let path = sidecar_path(media_path);
    // ponytail: checked before the rename, so a sidecar another app writes
    // in between is still replaced; closing that needs a lock no app shares.
    let exists = match fs::read(&path) {
        Ok(bytes) => {
            let owned = String::from_utf8(bytes).is_ok_and(|xml| owner::owned_by_lumina(&xml));
            if !owned {
                return Ok(SidecarOutcome::SkippedForeign);
            }
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => {
            log::debug!("could not read sidecar {path:?}, leaving it: {err}");
            return Ok(SidecarOutcome::SkippedForeign);
        }
    };
    if metadata.is_empty() {
        if exists {
            fs::remove_file(&path).with_context(|| format!("could not remove {path:?}"))?;
            return Ok(SidecarOutcome::Removed);
        }
        return Ok(SidecarOutcome::Unchanged);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("xmp.tmp");
    fs::write(&tmp, render_xmp(metadata)).with_context(|| format!("could not write {tmp:?}"))?;
    fs::rename(&tmp, &path).with_context(|| format!("could not finalize {path:?}"))?;
    Ok(SidecarOutcome::Written(path))
}

/// [`write_sidecar`] for a sidecar inside the library: also sets the macOS
/// hidden flag so Finder doesn't show one beside every item. The file stays
/// next to its media, where Lightroom and Bridge look for it. A failure to
/// hide is cosmetic and only logged (at debug: volumes without file flags,
/// such as exFAT or SMB, would fail on every write).
pub fn write_library_sidecar(
    media_path: &Path,
    metadata: &SidecarMetadata,
) -> Result<SidecarOutcome> {
    let outcome = write_sidecar(media_path, metadata)?;
    if let SidecarOutcome::Written(path) = &outcome {
        if let Err(err) = hide_in_finder(path) {
            log::debug!("could not hide sidecar {path:?}: {err:#}");
        }
    }
    Ok(outcome)
}

/// Sets `UF_HIDDEN`, keeping the file's other flags.
#[cfg(target_os = "macos")]
fn hide_in_finder(path: &Path) -> Result<()> {
    use std::ffi::CString;
    use std::os::macos::fs::MetadataExt;
    use std::os::unix::ffi::OsStrExt;
    const UF_HIDDEN: u32 = 0x8000;
    extern "C" {
        fn chflags(path: *const std::ffi::c_char, flags: u32) -> std::ffi::c_int;
    }
    let flags = fs::metadata(path)?.st_flags() | UF_HIDDEN;
    let c_path = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: `c_path` is a valid NUL-terminated string for the call's duration.
    if unsafe { chflags(c_path.as_ptr(), flags) } != 0 {
        return Err(std::io::Error::last_os_error()).context("chflags failed");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn hide_in_finder(_path: &Path) -> Result<()> {
    Ok(())
}

/// Reads a sidecar if present.
pub fn read_sidecar(media_path: &Path) -> Result<Option<SidecarMetadata>> {
    let path = sidecar_path(media_path);
    if !path.exists() {
        return Ok(None);
    }
    let xml = fs::read_to_string(&path).with_context(|| format!("could not read {path:?}"))?;
    Ok(Some(parse_xmp(&xml)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_media(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumina_xmp_{}_{}.jpg",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn written(outcome: Result<SidecarOutcome>) -> PathBuf {
        match outcome.unwrap() {
            SidecarOutcome::Written(path) => path,
            other => panic!("sidecar should be written, got {other:?}"),
        }
    }

    #[test]
    fn render_and_parse_roundtrip() {
        let metadata = SidecarMetadata {
            title: Some("A & B <title>".to_string()),
            caption: Some("Golden hour & <Kyoto>".to_string()),
            rating: 4,
            color_label: 2,
            tags: vec!["travel".to_string(), "japan & kyoto".to_string()],
            source_url: Some("https://example.com/video.mp4?a=1&b=2".to_string()),
            credit: Some("Channel & Artist".to_string()),
            ..Default::default()
        };
        let xml = render_xmp(&metadata);
        assert!(xml.contains("xmp:Rating=\"4\""));
        assert!(xml.contains("xmp:Label=\"Orange\""));
        assert!(xml.contains("travel"));
        assert!(xml.contains("<dc:source>https://example.com/video.mp4?a=1&amp;b=2</dc:source>"));
        assert!(xml.contains("<photoshop:Credit>Channel &amp; Artist</photoshop:Credit>"));
        assert_eq!(parse_xmp(&xml), metadata);
    }

    #[test]
    fn a_sidecar_never_carries_characters_xml_cannot_hold() {
        let metadata = SidecarMetadata {
            title: Some("A\u{FFFF}B\u{1}".to_string()),
            caption: Some("Line one\nLine two".to_string()),
            tags: vec!["x\u{0}y".to_string()],
            ..Default::default()
        };
        let xml = render_xmp(&metadata);
        assert!(
            !xml.chars()
                .any(|c| c == '\u{FFFF}' || c == '\u{1}' || c == '\u{0}'),
            "values stored before input was cleaned must still write valid XML"
        );
        assert!(
            xml.contains("Line one\nLine two"),
            "caption paragraphs survive"
        );
    }

    #[test]
    fn write_read_and_remove_sidecar() {
        let media = temp_media("roundtrip");
        fs::write(&media, b"jpeg").unwrap();
        let metadata = SidecarMetadata {
            title: Some("A title".to_string()),
            caption: Some("A caption".to_string()),
            rating: 3,
            color_label: 0,
            tags: vec!["one".to_string(), "two".to_string()],
            ..Default::default()
        };
        let sidecar = written(write_sidecar(&media, &metadata));
        assert!(sidecar.exists());
        assert_eq!(read_sidecar(&media).unwrap(), Some(metadata));

        assert_eq!(
            write_sidecar(&media, &SidecarMetadata::default()).unwrap(),
            SidecarOutcome::Removed
        );
        assert!(!sidecar.exists());

        let _ = fs::remove_file(&media);
    }

    #[test]
    fn a_label_alone_writes_a_sidecar_and_unknown_names_read_as_none() {
        let label_only = SidecarMetadata {
            color_label: 6,
            ..Default::default()
        };
        assert!(!label_only.is_empty(), "a color label alone is metadata");
        assert_eq!(parse_xmp(&render_xmp(&label_only)), label_only);
        assert_eq!(parse_xmp(r#"<x xmp:Label="green"/>"#).color_label, 4);
        assert_eq!(parse_xmp(r#"<x xmp:Label="To Print"/>"#).color_label, 0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn library_sidecars_are_hidden_from_finder() {
        use std::os::macos::fs::MetadataExt;
        let media = temp_media("hidden");
        fs::write(&media, b"jpeg").unwrap();
        let metadata = SidecarMetadata {
            rating: 4,
            ..Default::default()
        };
        let sidecar = written(write_library_sidecar(&media, &metadata));
        let flags = fs::metadata(&sidecar).unwrap().st_flags();
        assert_ne!(flags & 0x8000, 0, "a library sidecar must carry UF_HIDDEN");
        // A rewrite replaces the file; it must come back hidden too.
        write_library_sidecar(
            &media,
            &SidecarMetadata {
                rating: 5,
                ..metadata
            },
        )
        .unwrap();
        let flags = fs::metadata(&sidecar).unwrap().st_flags();
        assert_ne!(flags & 0x8000, 0, "a rewritten sidecar must stay hidden");
        let exported = written(write_sidecar(
            &media,
            &SidecarMetadata {
                rating: 2,
                ..Default::default()
            },
        ));
        assert_eq!(
            fs::metadata(&exported).unwrap().st_flags() & 0x8000,
            0,
            "plain (export) sidecars stay visible"
        );
        let _ = fs::remove_file(&sidecar);
        let _ = fs::remove_file(&media);
    }

    fn full_rights() -> Rights {
        Rights {
            creator: Some("Ada & Co".to_string()),
            copyright: Some("© 2026 Ada <ada>".to_string()),
            usage_terms: Some("Editorial use only".to_string()),
            contact_email: Some("ada@example.com".to_string()),
            contact_url: Some("https://ada.example/?a=1&b=2".to_string()),
        }
    }

    #[test]
    fn rights_roundtrip_through_xmp() {
        let metadata = SidecarMetadata {
            rating: 2,
            tags: vec!["travel".to_string()],
            rights: full_rights(),
            ..Default::default()
        };
        let xml = render_xmp(&metadata);
        for expected in [
            "<dc:creator>\n    <rdf:Seq>\n     <rdf:li>Ada &amp; Co</rdf:li>",
            "<dc:rights>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">© 2026 Ada &lt;ada&gt;</rdf:li>",
            "<xmpRights:UsageTerms>\n    <rdf:Alt>",
            "<Iptc4xmpCore:CreatorContactInfo rdf:parseType=\"Resource\">",
            "<Iptc4xmpCore:CiEmailWork>ada@example.com</Iptc4xmpCore:CiEmailWork>",
            "xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"",
            "xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\"",
        ] {
            assert!(xml.contains(expected), "missing {expected:?} in {xml}");
        }
        assert_eq!(parse_xmp(&xml), metadata, "every rights field reads back");
        assert!(
            !render_xmp(&SidecarMetadata::default()).contains("xmpRights"),
            "no rights, no rights namespaces"
        );
    }

    #[test]
    fn rights_only_sidecar_is_kept() {
        let media = temp_media("rights_only");
        fs::write(&media, b"jpeg").unwrap();
        let creator_only = SidecarMetadata {
            rights: Rights {
                creator: Some("Ada".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!creator_only.is_empty(), "a creator alone is metadata");
        let sidecar = written(write_sidecar(&media, &creator_only));
        assert!(
            sidecar.exists(),
            "a sidecar holding only a creator must not be deleted as empty"
        );
        assert_eq!(read_sidecar(&media).unwrap(), Some(creator_only));
        let _ = fs::remove_file(&sidecar);
        let _ = fs::remove_file(&media);
    }

    fn wall_time(text: &str) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn edited_date_roundtrips_through_xmp() {
        let metadata = SidecarMetadata {
            rating: 1,
            date_taken: Some(wall_time("2024-05-01 10:00:07")),
            ..Default::default()
        };
        let xml = render_xmp(&metadata);
        for expected in [
            "exif:DateTimeOriginal=\"2024-05-01T10:00:07\"",
            "photoshop:DateCreated=\"2024-05-01T10:00:07\"",
            "xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"",
        ] {
            assert!(xml.contains(expected), "missing {expected:?} in {xml}");
        }
        assert_eq!(parse_xmp(&xml), metadata, "the date reads back as written");
        let plain = render_xmp(&SidecarMetadata {
            rating: 1,
            ..Default::default()
        });
        assert!(
            !plain.contains("exif:") && !plain.contains("DateCreated"),
            "no date, no date names: {plain}"
        );
    }

    #[test]
    fn offset_dates_import_as_wall_time() {
        for (xml, expected) in [
            (
                r#"<x exif:DateTimeOriginal="2024-05-01T10:00:00+02:00"/>"#,
                "2024-05-01 10:00:00",
            ),
            (
                r#"<x exif:DateTimeOriginal="2024-05-01T10:00:00.250Z"/>"#,
                "2024-05-01 10:00:00",
            ),
            (
                "<x><exif:DateTimeOriginal>2024-05-01T10:00-05:00</exif:DateTimeOriginal></x>",
                "2024-05-01 10:00:00",
            ),
            (
                r#"<x photoshop:DateCreated="2024-05-01"/>"#,
                "2024-05-01 00:00:00",
            ),
            (
                r#"<x photoshop:DateCreated="2020-01-01" exif:DateTimeOriginal="2024-05-01T10:00:00"/>"#,
                "2024-05-01 10:00:00",
            ),
        ] {
            assert_eq!(
                parse_xmp(xml).date_taken,
                Some(wall_time(expected)),
                "an offset is dropped and exif:DateTimeOriginal wins: {xml}"
            );
        }
        assert_eq!(
            parse_xmp(r#"<x exif:DateTimeOriginal="soon"/>"#).date_taken,
            None
        );
        assert!(
            parse_xmp(r#"<x photoshop:DateCreated="2024-05-01"/>"#).date_only,
            "a date without a time is reported as date-only"
        );
        assert!(
            !parse_xmp(r#"<x photoshop:DateCreated="2024-05-01T00:00:00"/>"#).date_only,
            "an explicit midnight is a time"
        );
    }

    #[test]
    fn exif_format_dates_parse() {
        // darktable writes exif:DateTimeOriginal in EXIF's own format.
        assert_eq!(
            parse_xmp(
                r#"<x exif:DateTimeOriginal="2024:05:01 10:00:00" photoshop:DateCreated="2020-01-01"/>"#
            )
            .date_taken,
            Some(wall_time("2024-05-01 10:00:00")),
            "an EXIF-format date parses and wins over photoshop:DateCreated"
        );
    }

    #[test]
    fn gps_minutes_and_seconds_out_of_range_are_refused() {
        for lat in [
            "48,60.0N",
            "48,-1.0N",
            "48,10,60N",
            "48,10,-1N",
            "-48,10.0N",
        ] {
            assert_eq!(
                parse_xmp(&format!(
                    r#"<x exif:GPSLatitude="{lat}" exif:GPSLongitude="2,21.1E"/>"#
                ))
                .location,
                None,
                "{lat} is not a valid GPS coordinate"
            );
        }
    }

    #[test]
    fn gps_roundtrip_within_tolerance() {
        for (lat, lon) in [
            (48.856667, 2.352222),
            (-33.868820, 151.209296),
            (40.712776, -74.005974),
            (-22.906847, -43.172897),
            (0.0, 0.0),
            (89.9999999, -179.9999999),
        ] {
            let xml = render_xmp(&SidecarMetadata {
                location: Some((lat, lon)),
                ..Default::default()
            });
            let (read_lat, read_lon) = parse_xmp(&xml).location.expect("GPS reads back");
            assert!(
                (read_lat - lat).abs() < 1e-6 && (read_lon - lon).abs() < 1e-6,
                "({lat}, {lon}) came back as ({read_lat}, {read_lon}): {xml}"
            );
        }
        let paris = render_xmp(&SidecarMetadata {
            location: Some((48.856667, 2.352222)),
            ..Default::default()
        });
        assert!(
            paris.contains("exif:GPSLatitude=\"48,51.400020N\"")
                && paris.contains("exif:GPSLongitude=\"2,21.133320E\""),
            "XMP writes DDD,MM.mmmmmmK: {paris}"
        );
        let south_west = parse_xmp(
            r#"<x exif:GPSLatitude="33,52,7.75S"><exif:GPSLongitude>70,30.5W</exif:GPSLongitude></x>"#,
        )
        .location
        .expect("DDD,MM,SSK and element form parse");
        assert!((south_west.0 + (33.0 + 52.0 / 60.0 + 7.75 / 3600.0)).abs() < 1e-9);
        assert!((south_west.1 + (70.0 + 30.5 / 60.0)).abs() < 1e-9);
        assert_eq!(
            parse_xmp(r#"<x exif:GPSLatitude="48,51.4N"/>"#).location,
            None,
            "a latitude without a longitude is no position"
        );
        assert_eq!(
            parse_xmp(r#"<x exif:GPSLatitude="95,0.0N" exif:GPSLongitude="2,0.0E"/>"#).location,
            None,
            "an out-of-range latitude is refused"
        );
    }

    #[test]
    fn date_only_sidecar_is_kept() {
        let media = temp_media("date_only");
        fs::write(&media, b"jpeg").unwrap();
        let date_only = SidecarMetadata {
            date_taken: Some(wall_time("2019-07-04 18:30:00")),
            ..Default::default()
        };
        assert!(!date_only.is_empty(), "an edited date alone is metadata");
        let place_only = SidecarMetadata {
            location: Some((1.0, 2.0)),
            ..Default::default()
        };
        assert!(!place_only.is_empty(), "an edited place alone is metadata");
        let sidecar = written(write_sidecar(&media, &date_only));
        assert!(
            sidecar.exists(),
            "a sidecar holding only an edited date must not be deleted as empty"
        );
        assert_eq!(read_sidecar(&media).unwrap(), Some(date_only));
        let _ = fs::remove_file(&sidecar);
        let _ = fs::remove_file(&media);
    }

    #[test]
    fn missing_sidecar_is_none() {
        let media = temp_media("missing");
        assert_eq!(read_sidecar(&media).unwrap(), None);
    }
}
