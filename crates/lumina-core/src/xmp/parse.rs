//! Sidecar parsing: the subset of XMP Lumina reads, string-based (no XML crate).

use super::{SidecarMetadata, LABEL_NAMES};
use crate::catalog::Rights;

fn unescape_xml(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
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
        rights: parse_rights(xml),
    }
}

/// dc:creator (every name, joined with `; `), dc:rights,
/// xmpRights:UsageTerms and the work email and URL of
/// Iptc4xmpCore:CreatorContactInfo, as elements or (Lightroom) attributes.
fn parse_rights(xml: &str) -> Rights {
    let present = |value: String| Some(value).filter(|value| !value.trim().is_empty());
    let first_li = |name: &str| {
        between(xml, &format!("<{name}>"), &format!("</{name}>"))
            .and_then(|section| extract_li_values(section).into_iter().next())
    };
    let contact = |name: &str| {
        between(xml, &format!("<{name}>"), &format!("</{name}>"))
            .or_else(|| between(xml, &format!("{name}=\""), "\""))
            .map(|value| unescape_xml(value.trim()))
            .and_then(present)
    };
    Rights {
        creator: between(xml, "<dc:creator>", "</dc:creator>")
            .map(|section| extract_li_values(section).join("; "))
            .and_then(present),
        copyright: first_li("dc:rights").and_then(present),
        usage_terms: first_li("xmpRights:UsageTerms").and_then(present),
        contact_email: contact("Iptc4xmpCore:CiEmailWork"),
        contact_url: contact("Iptc4xmpCore:CiUrlWork"),
    }
    .cleaned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xmp::render_xmp;

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
    fn several_creators_round_trip_as_separate_entries() {
        let metadata = SidecarMetadata {
            rights: crate::catalog::Rights {
                creator: Some("Ada Lovelace; Alan Turing".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let xml = crate::xmp::render::render_xmp(&metadata);
        assert_eq!(
            xml.matches("<rdf:li>").count(),
            2,
            "each creator gets its own rdf:li: {xml}"
        );
        assert_eq!(
            parse_xmp(&xml).rights.creator.as_deref(),
            Some("Ada Lovelace; Alan Turing")
        );
    }

    #[test]
    fn lightroom_contact_attributes_parse() {
        // Lightroom writes the contact struct's fields as attributes.
        let xml = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmpRights="http://ns.adobe.com/xap/1.0/rights/"
    xmlns:Iptc4xmpCore="http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/">
   <dc:creator><rdf:Seq><rdf:li>Ada</rdf:li><rdf:li>Bea</rdf:li></rdf:Seq></dc:creator>
   <dc:rights><rdf:Alt><rdf:li xml:lang="x-default">© Ada &amp; Bea</rdf:li></rdf:Alt></dc:rights>
   <Iptc4xmpCore:CreatorContactInfo
    Iptc4xmpCore:CiEmailWork="ada@example.com"
    Iptc4xmpCore:CiUrlWork="https://ada.example/?a=1&amp;b=2"/>
  </rdf:Description></rdf:RDF></x:xmpmeta>"#;
        let rights = parse_xmp(xml).rights;
        assert_eq!(
            rights.creator.as_deref(),
            Some("Ada; Bea"),
            "every creator is kept"
        );
        assert_eq!(rights.copyright.as_deref(), Some("© Ada & Bea"));
        assert_eq!(rights.usage_terms, None);
        assert_eq!(
            rights.contact_email.as_deref(),
            Some("ada@example.com"),
            "attribute-form contact fields must parse"
        );
        assert_eq!(
            rights.contact_url.as_deref(),
            Some("https://ada.example/?a=1&b=2")
        );
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
