//! Sidecar rendering: the RDF/XML packet Lumina writes.

use super::{SidecarMetadata, LABEL_NAMES};

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
