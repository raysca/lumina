//! XMP sidecar persistence for catalog metadata.
//!
//! Ratings, captions and tags live in SQLite, but that data is trapped in
//! Lumina unless it is also written next to the media. We emit a small RDF/XML
//! sidecar (photo.jpg.xmp) compatible with Adobe conventions for
//! xmp:Rating, dc:description and dc:subject, and can read it back.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SidecarMetadata {
    pub caption: Option<String>,
    pub rating: u8,
    pub tags: Vec<String>,
}

impl SidecarMetadata {
    pub fn is_empty(&self) -> bool {
        self.caption.as_deref().unwrap_or("").trim().is_empty()
            && self.rating == 0
            && self.tags.is_empty()
    }
}

/// photo.jpg -> photo.jpg.xmp
pub fn sidecar_path(media_path: &Path) -> PathBuf {
    let mut path = media_path.as_os_str().to_owned();
    path.push(".xmp");
    PathBuf::from(path)
}

fn escape_xml(value: &str) -> String {
    value
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
        .replace("&amp;", "&")
}

/// Builds the RDF/XML sidecar body for the metadata.
pub fn render_xmp(metadata: &SidecarMetadata) -> String {
    let mut subject = String::new();
    if !metadata.tags.is_empty() {
        subject.push_str("   <dc:subject>\n    <rdf:Bag>\n");
        for tag in &metadata.tags {
            subject.push_str(&format!("     <rdf:li>{}</rdf:li>\n", escape_xml(tag)));
        }
        subject.push_str("    </rdf:Bag>\n   </dc:subject>\n");
    }

    let description = match metadata.caption.as_deref() {
        Some(caption) if !caption.trim().is_empty() => format!(
            "   <dc:description>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </dc:description>\n",
            escape_xml(caption)
        ),
        _ => String::new(),
    };

    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n    xmp:Rating=\"{}\"\n  >\n{}{}  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n",
        metadata.rating, subject, description
    )
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

    let caption = between(xml, "<dc:description>", "</dc:description>")
        .and_then(|section| extract_li_values(section).into_iter().next())
        .filter(|value| !value.trim().is_empty());

    let tags = between(xml, "<dc:subject>", "</dc:subject>")
        .map(extract_li_values)
        .unwrap_or_default();

    SidecarMetadata {
        caption,
        rating,
        tags,
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
            caption: Some("Golden hour & <Kyoto>".to_string()),
            rating: 4,
            tags: vec!["travel".to_string(), "japan & kyoto".to_string()],
        };
        let xml = render_xmp(&metadata);
        assert!(xml.contains("xmp:Rating=\"4\""));
        assert!(xml.contains("travel"));
        assert_eq!(parse_xmp(&xml), metadata);
    }

    #[test]
    fn write_read_and_remove_sidecar() {
        let media = temp_media("roundtrip");
        fs::write(&media, b"jpeg").unwrap();
        let metadata = SidecarMetadata {
            caption: Some("A caption".to_string()),
            rating: 3,
            tags: vec!["one".to_string(), "two".to_string()],
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
    fn missing_sidecar_is_none() {
        let media = temp_media("missing");
        assert_eq!(read_sidecar(&media).unwrap(), None);
    }
}
