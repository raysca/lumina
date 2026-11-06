//! XMP sidecar persistence for catalog metadata.
//!
//! Ratings, color labels, captions and tags live in SQLite, but that data is trapped in
//! Lumina unless it is also written next to the media. We emit a small RDF/XML
//! sidecar (photo.jpg.xmp) compatible with Adobe conventions for
//! xmp:Rating, xmp:Label, dc:description, dc:subject and lr:hierarchicalSubject
//! (keyword paths), and can read it back.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SidecarMetadata {
    pub title: Option<String>,
    pub caption: Option<String>,
    pub rating: u8,
    /// 0 = none, 1..=6 = [`LABEL_NAMES`] (the catalog's `color_label`).
    pub color_label: u8,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    pub credit: Option<String>,
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
    }
}

/// photo.jpg -> photo.jpg.xmp
pub fn sidecar_path(media_path: &Path) -> PathBuf {
    let mut path = media_path.as_os_str().to_owned();
    path.push(".xmp");
    PathBuf::from(path)
}

/// Also drops characters XML cannot hold, so a value stored before input
/// was cleaned still writes a readable sidecar.
fn escape_xml(value: &str) -> String {
    value
        .chars()
        .filter(|&c| !crate::text::is_disallowed(c) || matches!(c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Builds the RDF/XML sidecar body for the metadata.
pub fn render_xmp(metadata: &SidecarMetadata) -> String {
    // dc:subject holds every path segment once (the flat keywords Lightroom
    // and Bridge show); lr:hierarchicalSubject holds every tag, flat ones too,
    // so a flat `europe` beside `places/europe` is not taken for a segment on
    // re-import. A tag with `|` cannot be written in Lightroom's syntax and
    // stays flat in dc:subject only.
    let mut segments: Vec<&str> = Vec::new();
    let mut hierarchy = Vec::new();
    for tag in &metadata.tags {
        if tag.contains('|') {
            if !segments.contains(&tag.as_str()) {
                segments.push(tag);
            }
            continue;
        }
        hierarchy.push(tag.replace('/', "|"));
        for segment in tag.split('/').filter(|segment| !segment.is_empty()) {
            if !segments.contains(&segment) {
                segments.push(segment);
            }
        }
    }
    let subject = bag("dc:subject", segments);
    let hierarchical = bag(
        "lr:hierarchicalSubject",
        hierarchy.iter().map(String::as_str),
    );
    let lr_namespace = if hierarchy.is_empty() {
        ""
    } else {
        "    xmlns:lr=\"http://ns.adobe.com/lightroom/1.0/\"\n"
    };

    let title = match metadata.title.as_deref() {
        Some(title) if !title.trim().is_empty() => format!(
            "   <dc:title>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </dc:title>\n",
            escape_xml(title)
        ),
        _ => String::new(),
    };

    let description = match metadata.caption.as_deref() {
        Some(caption) if !caption.trim().is_empty() => format!(
            "   <dc:description>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </dc:description>\n",
            escape_xml(caption)
        ),
        _ => String::new(),
    };

    let label = match metadata.color_label {
        n @ 1..=6 => format!("    xmp:Label=\"{}\"\n", LABEL_NAMES[n as usize - 1]),
        _ => String::new(),
    };

    let source = match metadata.source_url.as_deref() {
        Some(source) if !source.trim().is_empty() => {
            format!("   <dc:source>{}</dc:source>\n", escape_xml(source))
        }
        _ => String::new(),
    };

    let credit = match metadata.credit.as_deref() {
        Some(credit) if !credit.trim().is_empty() => format!(
            "   <photoshop:Credit>{}</photoshop:Credit>\n",
            escape_xml(credit)
        ),
        _ => String::new(),
    };

    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n{}    xmp:Rating=\"{}\"\n{}  >\n{}{}{}{}{}{}  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n",
        lr_namespace, metadata.rating, label, title, subject, hierarchical, description, source, credit
    )
}

/// An `rdf:Bag` element named `name`, or nothing when `values` is empty.
fn bag<'a>(name: &str, values: impl IntoIterator<Item = &'a str>) -> String {
    let items: String = values
        .into_iter()
        .map(|value| format!("     <rdf:li>{}</rdf:li>\n", escape_xml(value)))
        .collect();
    if items.is_empty() {
        return String::new();
    }
    format!("   <{name}>\n    <rdf:Bag>\n{items}    </rdf:Bag>\n   </{name}>\n")
}

fn between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = haystack.find(start)? + start.len();
    let rest = &haystack[start_idx..];
    let end_idx = rest.find(end)?;
    Some(&rest[..end_idx])
}

fn extract_li_values(section: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = section;
    while let Some(inner) = between(rest, "<rdf:li", "</rdf:li>") {
        if let Some(text_start) = inner.find('>') {
            let value = unescape_xml(inner[text_start + 1..].trim());
            if !value.is_empty() {
                values.push(value);
            }
        }
        let Some(consumed) = rest.find("</rdf:li>") else {
            break;
        };
        rest = &rest[consumed + "</rdf:li>".len()..];
    }
    values
}

/// Parses the subset of XMP that Lumina writes. Missing sections are tolerated.
pub fn parse_xmp(xml: &str) -> SidecarMetadata {
    let rating = between(xml, "xmp:Rating=\"", "\"")
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(0)
        .min(5);

    // Unknown names (a custom Lightroom label set) read as no label.
    let color_label = between(xml, "xmp:Label=\"", "\"")
        .and_then(|name| {
            LABEL_NAMES
                .iter()
                .position(|known| known.eq_ignore_ascii_case(name.trim()))
        })
        .map_or(0, |index| index as u8 + 1);

    let title = between(xml, "<dc:title>", "</dc:title>")
        .and_then(|section| extract_li_values(section).into_iter().next())
        .filter(|value| !value.trim().is_empty());

    let caption = between(xml, "<dc:description>", "</dc:description>")
        .and_then(|section| extract_li_values(section).into_iter().next())
        .filter(|value| !value.trim().is_empty());

    let subject = between(xml, "<dc:subject>", "</dc:subject>")
        .map(extract_li_values)
        .unwrap_or_default();
    let tags = match between(xml, "<lr:hierarchicalSubject>", "</lr:hierarchicalSubject>") {
        // Paths, plus the dc:subject values that are not a segment of one
        // (Lightroom lists every leaf and parent name there too).
        Some(section) => {
            let entries = extract_li_values(section);
            // Segments split on Lightroom's `|`, so a keyword such as
            // `AC/DC` in `Music|AC/DC` is still one segment.
            let segments: Vec<String> = entries
                .iter()
                .flat_map(|entry| entry.split('|'))
                .map(str::to_lowercase)
                .collect();
            let mut tags: Vec<String> = Vec::new();
            for entry in &entries {
                let path = entry.replace('|', "/");
                if !tags.contains(&path) {
                    tags.push(path);
                }
            }
            for value in subject {
                if !segments.contains(&value.to_lowercase()) && !tags.contains(&value) {
                    tags.push(value);
                }
            }
            tags
        }
        None => subject,
    };

    let source_url = between(xml, "<dc:source>", "</dc:source>")
        .map(|s| unescape_xml(s.trim()))
        .filter(|s| !s.is_empty());

    let credit = between(xml, "<photoshop:Credit>", "</photoshop:Credit>")
        .map(|s| unescape_xml(s.trim()))
        .filter(|s| !s.is_empty());

    SidecarMetadata {
        title,
        caption,
        rating,
        color_label,
        tags,
        source_url,
        credit,
    }
}

/// Writes the sidecar atomically. Empty metadata removes any existing sidecar.
pub fn write_sidecar(media_path: &Path, metadata: &SidecarMetadata) -> Result<Option<PathBuf>> {
    let path = sidecar_path(media_path);
    if metadata.is_empty() {
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("could not remove {path:?}"))?;
        }
        return Ok(None);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("xmp.tmp");
    fs::write(&tmp, render_xmp(metadata)).with_context(|| format!("could not write {tmp:?}"))?;
    fs::rename(&tmp, &path).with_context(|| format!("could not finalize {path:?}"))?;
    Ok(Some(path))
}

/// [`write_sidecar`] for a sidecar inside the library: also sets the macOS
/// hidden flag so Finder doesn't show one beside every item. The file stays
/// next to its media, where Lightroom and Bridge look for it. A failure to
/// hide is cosmetic and only logged (at debug: volumes without file flags,
/// such as exFAT or SMB, would fail on every write).
pub fn write_library_sidecar(
    media_path: &Path,
    metadata: &SidecarMetadata,
) -> Result<Option<PathBuf>> {
    let written = write_sidecar(media_path, metadata)?;
    if let Some(path) = &written {
        if let Err(err) = hide_in_finder(path) {
            log::debug!("could not hide sidecar {path:?}: {err:#}");
        }
    }
    Ok(written)
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
        let sidecar = write_sidecar(&media, &metadata)
            .unwrap()
            .expect("sidecar should be written");
        assert!(sidecar.exists());
        assert_eq!(read_sidecar(&media).unwrap(), Some(metadata));

        assert_eq!(
            write_sidecar(&media, &SidecarMetadata::default()).unwrap(),
            None
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
        let sidecar = write_library_sidecar(&media, &metadata)
            .unwrap()
            .expect("sidecar should be written");
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
        let exported = write_sidecar(
            &media,
            &SidecarMetadata {
                rating: 2,
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            fs::metadata(&exported).unwrap().st_flags() & 0x8000,
            0,
            "plain (export) sidecars stay visible"
        );
        let _ = fs::remove_file(&sidecar);
        let _ = fs::remove_file(&media);
    }

    #[test]
    fn missing_sidecar_is_none() {
        let media = temp_media("missing");
        assert_eq!(read_sidecar(&media).unwrap(), None);
    }

    fn tags_of(xml: &str) -> Vec<String> {
        let mut tags = parse_xmp(xml).tags;
        tags.sort();
        tags
    }

    fn with_tags(tags: &[&str]) -> SidecarMetadata {
        SidecarMetadata {
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn hierarchy_roundtrips_through_xmp() {
        let xml = render_xmp(&with_tags(&["places/europe/paris", "places/europe/lyon"]));
        assert!(
            xml.contains("xmlns:lr=\"http://ns.adobe.com/lightroom/1.0/\""),
            "the hierarchy needs the Lightroom namespace"
        );
        assert!(
            xml.contains("<rdf:li>places|europe|paris</rdf:li>"),
            "lr:hierarchicalSubject writes paths with |: {xml}"
        );
        let subject = between(&xml, "<dc:subject>", "</dc:subject>").unwrap();
        assert_eq!(
            extract_li_values(subject),
            vec!["places", "europe", "paris", "lyon"],
            "dc:subject holds each segment once, as Lightroom shows flat keywords"
        );
        assert_eq!(
            tags_of(&xml),
            vec!["places/europe/lyon", "places/europe/paris"],
            "paths come back as paths, segments are not extra tags"
        );
    }

    #[test]
    fn flat_tag_equal_to_a_segment_roundtrips() {
        let xml = render_xmp(&with_tags(&["europe", "places/europe"]));
        assert_eq!(
            tags_of(&xml),
            vec!["europe", "places/europe"],
            "a flat tag that is also a segment must not be dropped on re-import"
        );
    }

    #[test]
    fn flat_subject_without_hierarchy_imports_flat() {
        let xml = r#"<rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
          <dc:subject><rdf:Bag><rdf:li>travel</rdf:li><rdf:li>a/b</rdf:li></rdf:Bag></dc:subject>
          </rdf:Description>"#;
        assert_eq!(
            tags_of(xml),
            vec!["a/b", "travel"],
            "without lr:hierarchicalSubject every dc:subject value is a tag"
        );
    }

    #[test]
    fn lightroom_style_sidecar_imports_paths() {
        let xml = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="Adobe XMP Core 7.0-c000 1.000000, 0000/00/00-00:00:00        ">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:lr="http://ns.adobe.com/lightroom/1.0/"
   xmp:Rating="3">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>Paris</rdf:li>
     <rdf:li>Europe</rdf:li>
     <rdf:li>Places</rdf:li>
     <rdf:li>Lyon</rdf:li>
     <rdf:li>Europe</rdf:li>
     <rdf:li>sunset</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <lr:hierarchicalSubject>
    <rdf:Bag>
     <rdf:li>Places|Europe|Paris</rdf:li>
     <rdf:li>Places|Europe|Lyon</rdf:li>
     <rdf:li>sunset</rdf:li>
    </rdf:Bag>
   </lr:hierarchicalSubject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"#;
        assert_eq!(
            tags_of(xml),
            vec!["Places/Europe/Lyon", "Places/Europe/Paris", "sunset"],
            "leaf and parent names in dc:subject must not become duplicate flat tags"
        );
    }

    #[test]
    fn lightroom_keyword_with_a_slash_adds_no_flat_tag() {
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:lr="http://ns.adobe.com/lightroom/1.0/"><dc:subject><rdf:Bag><rdf:li>Music</rdf:li><rdf:li>AC/DC</rdf:li></rdf:Bag></dc:subject><lr:hierarchicalSubject><rdf:Bag><rdf:li>Music|AC/DC</rdf:li></rdf:Bag></lr:hierarchicalSubject></rdf:Description></rdf:RDF></x:xmpmeta>"#;
        assert_eq!(
            parse_xmp(xml).tags,
            vec!["Music/AC/DC".to_string()],
            "a dc:subject leaf that is a Lightroom segment must not import again as a flat tag"
        );
    }

    #[test]
    fn pipe_in_tag_stays_flat() {
        let xml = render_xmp(&with_tags(&["a|b", "x/y"]));
        let hierarchy = between(
            &xml,
            "<lr:hierarchicalSubject>",
            "</lr:hierarchicalSubject>",
        )
        .expect("x/y is written to the hierarchy");
        assert_eq!(
            extract_li_values(hierarchy),
            vec!["x|y"],
            "a tag with | cannot be expressed in Lightroom's syntax"
        );
        assert_eq!(tags_of(&xml), vec!["a|b", "x/y"], "the | tag stays flat");
    }

    #[test]
    fn unescapes_apostrophes_and_standard_xml_entities() {
        let xml = r#"
        <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
         <rdf:Description xmlns:dc="http://purl.org/dc/elements/1.1/">
          <dc:title>
           <rdf:Alt><rdf:li xml:lang="x-default">It&apos;s a dog &amp; cat &quot;story&quot;</rdf:li></rdf:Alt>
          </dc:title>
          <dc:subject>
           <rdf:Bag><rdf:li>rock &apos;n&apos; roll</rdf:li></rdf:Bag>
          </dc:subject>
         </rdf:Description>
        </rdf:RDF>"#;
        let parsed = parse_xmp(xml);
        assert_eq!(parsed.title.as_deref(), Some("It's a dog & cat \"story\""));
        assert_eq!(parsed.tags, vec!["rock 'n' roll"]);
    }
}
