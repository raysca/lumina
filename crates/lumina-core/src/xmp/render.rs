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

    let rights = metadata.rights.cleaned();
    let creator = match rights.creator.as_deref() {
        // Import joins several creators with "; ", so split them back into
        // one rdf:li each and a multi-creator sidecar round-trips.
        Some(creator) => {
            let items: String = creator
                .split("; ")
                .filter(|name| !name.trim().is_empty())
                .map(|name| format!("     <rdf:li>{}</rdf:li>\n", escape_xml(name.trim())))
                .collect();
            format!("   <dc:creator>\n    <rdf:Seq>\n{items}    </rdf:Seq>\n   </dc:creator>\n")
        }
        None => String::new(),
    };
    let copyright = alt("dc:rights", rights.copyright.as_deref());
    let usage_terms = alt("xmpRights:UsageTerms", rights.usage_terms.as_deref());
    let contact_fields: String = [
        ("Iptc4xmpCore:CiEmailWork", rights.contact_email.as_deref()),
        ("Iptc4xmpCore:CiUrlWork", rights.contact_url.as_deref()),
    ]
    .into_iter()
    .filter_map(|(name, value)| {
        value.map(|value| format!("    <{name}>{}</{name}>\n", escape_xml(value)))
    })
    .collect();
    let contact = if contact_fields.is_empty() {
        String::new()
    } else {
        format!(
            "   <Iptc4xmpCore:CreatorContactInfo rdf:parseType=\"Resource\">\n{contact_fields}   </Iptc4xmpCore:CreatorContactInfo>\n"
        )
    };
    let mut rights_namespaces = String::new();
    if rights.usage_terms.is_some() {
        rights_namespaces.push_str("    xmlns:xmpRights=\"http://ns.adobe.com/xap/1.0/rights/\"\n");
    }
    if !contact.is_empty() {
        rights_namespaces
            .push_str("    xmlns:Iptc4xmpCore=\"http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/\"\n");
    }

    // Only an edited date or place reaches the sidecar (the engine leaves
    // them `None` otherwise), as wall time without an offset.
    let mut exif = String::new();
    if let Some(date) = metadata.date_taken {
        let date = date.format("%Y-%m-%dT%H:%M:%S");
        exif.push_str(&format!(
            "    exif:DateTimeOriginal=\"{date}\"\n    photoshop:DateCreated=\"{date}\"\n"
        ));
    }
    if let Some((lat, lon)) = metadata.location {
        exif.push_str(&format!(
            "    exif:GPSLatitude=\"{}\"\n    exif:GPSLongitude=\"{}\"\n",
            gps_coordinate(lat, 'N', 'S'),
            gps_coordinate(lon, 'E', 'W')
        ));
    }
    if !exif.is_empty() {
        rights_namespaces.push_str("    xmlns:exif=\"http://ns.adobe.com/exif/1.0/\"\n");
    }

    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  <rdf:Description rdf:about=\"\"\n    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n    xmlns:photoshop=\"http://ns.adobe.com/photoshop/1.0/\"\n    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n{}{}    xmp:Rating=\"{}\"\n{}{}  >\n{}{}{}{}{}{}{}{}{}{}  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n",
        lr_namespace,
        rights_namespaces,
        metadata.rating,
        label,
        exif,
        title,
        subject,
        hierarchical,
        description,
        source,
        credit,
        creator,
        copyright,
        usage_terms,
        contact
    )
}

/// XMP's GPSCoordinate `DDD,MM.mmmmmmK`: six decimals of a minute is about
/// 2 mm. Rounded once in whole micro-minutes, so 59.9999999' carries into
/// the degree instead of printing 60.000000.
fn gps_coordinate(degrees: f64, positive: char, negative: char) -> String {
    let hemisphere = if degrees < 0.0 { negative } else { positive };
    let micro_minutes = (degrees.abs() * 60_000_000.0).round() as u64;
    format!(
        "{},{:02}.{:06}{hemisphere}",
        micro_minutes / 60_000_000,
        micro_minutes % 60_000_000 / 1_000_000,
        micro_minutes % 1_000_000
    )
}

/// A language-alternative element (`rdf:Alt`, `x-default`), or nothing.
fn alt(name: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => format!(
            "   <{name}>\n    <rdf:Alt>\n     <rdf:li xml:lang=\"x-default\">{}</rdf:li>\n    </rdf:Alt>\n   </{name}>\n",
            escape_xml(value)
        ),
        None => String::new(),
    }
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
