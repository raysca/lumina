//! Sidecar ownership: Lumina rewrites or deletes a `photo.jpg.xmp` only if
//! it could have written it. darktable and digiKam use the same file name,
//! and their sidecars hold edit history Lumina cannot carry over.

/// Every qualified element and attribute name any released Lumina version
/// has written (git `4a94c34`, `73480ad`, `c76f15e`, `0147095`, the `lr:`
/// names since keyword paths, the rights names since schema v17, and the
/// edited date and GPS names). A name
/// allow-list, so sidecars from older versions need no marker. Extend it
/// when the renderer gains a name.
const KNOWN_NAMES: &[&str] = &[
    "x:xmpmeta",
    "xmlns:x",
    "rdf:RDF",
    "xmlns:rdf",
    "rdf:Description",
    "rdf:about",
    "xmlns:dc",
    "xmlns:photoshop",
    "xmlns:xmp",
    "xmlns:lr",
    "xmlns:xmpRights",
    "xmlns:Iptc4xmpCore",
    "xmp:Rating",
    "xmp:Label",
    "dc:title",
    "dc:description",
    "dc:subject",
    "dc:source",
    "photoshop:Credit",
    "lr:hierarchicalSubject",
    "dc:creator",
    "dc:rights",
    "xmpRights:UsageTerms",
    "Iptc4xmpCore:CreatorContactInfo",
    "Iptc4xmpCore:CiEmailWork",
    "Iptc4xmpCore:CiUrlWork",
    "xmlns:exif",
    "exif:DateTimeOriginal",
    "exif:GPSLatitude",
    "exif:GPSLongitude",
    "photoshop:DateCreated",
    "rdf:parseType",
    "rdf:Seq",
    "rdf:Alt",
    "rdf:Bag",
    "rdf:li",
    "xml:lang",
];

/// True when every element and attribute name in `xml` is one Lumina writes
/// and the file is a Lumina packet (or blank: nothing to lose).
///
/// Only markup is scanned: Lumina escapes `<` in values but not `:` or `=`,
/// so a caption such as `ratio a:b=2` is text, not a name. Processing
/// instructions (`<?xpacket …?>`) are skipped; comments, CDATA and DOCTYPE
/// make the file foreign because Lumina never writes them. A NUL means
/// UTF-16 read as bytes (Lumina has stripped NULs from values since
/// 1e82c76; an older sidecar holding one is treated as foreign, which is
/// safe), so it is foreign.
pub(crate) fn owned_by_lumina(xml: &str) -> bool {
    if xml.contains('\0') {
        return false;
    }
    if xml.trim().is_empty() {
        return true;
    }
    let mut saw_packet = false;
    let mut rest = xml;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(instruction) = rest.strip_prefix('?') {
            let Some(end) = instruction.find("?>") else {
                return false;
            };
            rest = &instruction[end + 2..];
            continue;
        }
        if rest.starts_with('!') {
            return false;
        }
        // The tag up to its `>`, with quoted attribute values dropped: what
        // is left splits into the element name and the attribute names.
        let mut quote = None;
        let mut names = String::new();
        let mut end = None;
        for (index, c) in rest.char_indices() {
            match quote {
                Some(open) if c == open => quote = None,
                Some(_) => {}
                None => match c {
                    '"' | '\'' => quote = Some(c),
                    '>' => {
                        end = Some(index);
                        break;
                    }
                    _ => names.push(c),
                },
            }
        }
        let Some(end) = end else {
            return false;
        };
        rest = &rest[end + 1..];
        for name in names
            .split(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .filter(|name| !name.is_empty())
        {
            if !KNOWN_NAMES.contains(&name) {
                return false;
            }
            saw_packet |= name == "x:xmpmeta";
        }
    }
    saw_packet
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xmp::{render_xmp, write_sidecar, SidecarMetadata, SidecarOutcome};
    use std::fs;
    use std::path::PathBuf;

    /// darktable writes its edit history into `photo.jpg.xmp`.
    const DARKTABLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:darktable="http://darktable.sf.net/"
   xmp:Rating="1"
   darktable:xmp_version="5"
   darktable:history_end="2">
   <darktable:history>
    <rdf:Seq>
     <rdf:li darktable:num="0" darktable:operation="exposure" darktable:enabled="1"/>
     <rdf:li darktable:num="1" darktable:operation="crop" darktable:enabled="1"/>
    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;

    fn temp_media(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lumina_xmp_owner_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join("photo.jpg")
    }

    fn rated() -> SidecarMetadata {
        SidecarMetadata {
            rating: 5,
            ..Default::default()
        }
    }

    /// Writes `bytes` as the sidecar, runs `write_sidecar` with `metadata`
    /// and returns the outcome and the sidecar's bytes afterwards.
    fn write_over(
        name: &str,
        bytes: &[u8],
        metadata: &SidecarMetadata,
    ) -> (SidecarOutcome, Option<Vec<u8>>) {
        let media = temp_media(name);
        let sidecar = crate::xmp::sidecar_path(&media);
        fs::write(&sidecar, bytes).unwrap();
        let outcome = write_sidecar(&media, metadata).unwrap();
        let after = fs::read(&sidecar).ok();
        fs::remove_dir_all(media.parent().unwrap()).unwrap();
        (outcome, after)
    }

    #[test]
    fn legacy_lumina_sidecars_are_owned() {
        let full = SidecarMetadata {
            title: Some("Kyoto".into()),
            caption: Some("Golden hour & temples".into()),
            rating: 4,
            color_label: 2,
            tags: vec!["travel".into(), "places/japan/kyoto".into()],
            source_url: Some("https://example.com/v.mp4?a=1&b=2".into()),
            credit: Some("Channel".into()),
            rights: crate::catalog::Rights {
                creator: Some("Ada".into()),
                copyright: Some("© 2026 Ada".into()),
                usage_terms: Some("Editorial use only".into()),
                contact_email: Some("ada@example.com".into()),
                contact_url: Some("https://ada.example".into()),
            },
            date_taken: chrono::NaiveDate::from_ymd_opt(2024, 5, 1)
                .and_then(|day| day.and_hms_opt(10, 0, 0)),
            location: Some((-33.86882, 151.209296)),
            date_only: false,
        };
        let today = render_xmp(&full);
        assert!(
            today.contains("exif:GPSLatitude") && today.contains("photoshop:DateCreated"),
            "the full render carries date and GPS: {today}"
        );
        // One packet per released render version, as each one wrote it.
        let legacy = [
            // 4a94c34
            concat!(
                "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="4"
  >
   <dc:subject>
    <rdf:Bag>
     <rdf:li>travel</rdf:li>
     <rdf:li>kyoto</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <dc:description>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Golden hour &amp; temples</rdf:li>
    </rdf:Alt>
   </dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#
            ),
            // 73480ad
            concat!(
                "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="4"
  >
   <dc:title>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Kyoto</rdf:li>
    </rdf:Alt>
   </dc:title>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>travel</rdf:li>
     <rdf:li>kyoto</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <dc:description>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Golden hour &amp; temples</rdf:li>
    </rdf:Alt>
   </dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#
            ),
            // c76f15e
            concat!(
                "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="4"
    xmp:Label="Orange"
  >
   <dc:title>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Kyoto</rdf:li>
    </rdf:Alt>
   </dc:title>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>travel</rdf:li>
     <rdf:li>kyoto</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <dc:description>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Golden hour &amp; temples</rdf:li>
    </rdf:Alt>
   </dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#
            ),
            // 0147095
            concat!(
                "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n",
                r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmp:Rating="4"
    xmp:Label="Orange"
  >
   <dc:title>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Kyoto</rdf:li>
    </rdf:Alt>
   </dc:title>
   <dc:subject>
    <rdf:Bag>
     <rdf:li>travel</rdf:li>
     <rdf:li>kyoto</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <dc:description>
    <rdf:Alt>
     <rdf:li xml:lang="x-default">Golden hour &amp; temples</rdf:li>
    </rdf:Alt>
   </dc:description>
   <dc:source>https://example.com/v.mp4?a=1&amp;b=2</dc:source>
   <photoshop:Credit>Channel</photoshop:Credit>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#
            ),
        ];
        for (index, xml) in legacy.iter().chain([&today.as_str()]).enumerate() {
            assert!(
                owned_by_lumina(xml),
                "a sidecar from Lumina render version {index} must stay Lumina's: {xml}"
            );
        }
    }

    #[test]
    fn caption_text_with_colon_equals_stays_owned() {
        let xml = render_xmp(&SidecarMetadata {
            title: Some("x:y=\"1\" darktable:history=".into()),
            caption: Some("ratio a:b=2, see <darktable:history>".into()),
            tags: vec!["exif:Make=Canon".into()],
            ..Default::default()
        });
        assert!(
            owned_by_lumina(&xml),
            "names in element text are not markup and must not make the sidecar foreign: {xml}"
        );
    }

    #[test]
    fn unknown_attribute_makes_sidecar_foreign() {
        assert!(
            !owned_by_lumina(DARKTABLE),
            "darktable's history is not Lumina's"
        );
        let lumina = render_xmp(&rated());
        let with_toolkit = lumina.replace(
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">",
            "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"Adobe XMP Core 7.0\">",
        );
        assert!(
            !owned_by_lumina(&with_toolkit),
            "an unknown attribute (Adobe's x:xmptk) makes the sidecar foreign"
        );
        let with_element = lumina.replace(
            "  </rdf:Description>",
            "   <crs:Exposure2012>+0.5</crs:Exposure2012>\n  </rdf:Description>",
        );
        assert!(
            !owned_by_lumina(&with_element),
            "an unknown element makes it foreign"
        );
        let with_comment = lumina.replace(
            "  </rdf:Description>",
            "  <!-- kept -->\n  </rdf:Description>",
        );
        assert!(
            !owned_by_lumina(&with_comment),
            "Lumina never writes comments"
        );
        assert!(
            !owned_by_lumina("just some notes"),
            "a non-blank file with no XMP packet is not Lumina's"
        );
    }

    #[test]
    fn foreign_sidecars_with_exif_names_stay_foreign() {
        // darktable and digiKam write the same exif: date and GPS names
        // Lumina does, next to names of their own.
        let darktable = r#"<?xml version="1.0" encoding="UTF-8"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/" x:xmptk="XMP Core 4.4.0-Exiv2">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:darktable="http://darktable.sf.net/"
   exif:DateTimeOriginal="2024:05:01 10:00:00"
   exif:GPSLatitude="48,51.400020N"
   exif:GPSLongitude="2,21.133320E"
   xmp:Rating="1"
   darktable:xmp_version="5">
   <darktable:history>
    <rdf:Seq>
     <rdf:li darktable:operation="exposure" darktable:enabled="1"/>
    </rdf:Seq>
   </darktable:history>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
"#;
        let digikam = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about=""
    xmlns:exif="http://ns.adobe.com/exif/1.0/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/"
    xmlns:digiKam="http://www.digikam.org/ns/1.0/"
   exif:DateTimeOriginal="2024-05-01T10:00:00"
   exif:GPSLatitude="48,51.400020N"
   exif:GPSLongitude="2,21.133320E"
   photoshop:DateCreated="2024-05-01T10:00:00">
   <digiKam:TagsList>
    <rdf:Seq>
     <rdf:li>Places/Paris</rdf:li>
    </rdf:Seq>
   </digiKam:TagsList>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>
"#;
        for (name, xml) in [("darktable", darktable), ("digiKam", digikam)] {
            assert!(
                !owned_by_lumina(xml),
                "a {name} sidecar with exif: names must stay foreign"
            );
            let (outcome, after) = write_over(name, xml.as_bytes(), &rated());
            assert_eq!(outcome, SidecarOutcome::SkippedForeign, "{name}");
            assert_eq!(after.as_deref(), Some(xml.as_bytes()), "{name} untouched");
        }
    }

    #[test]
    fn foreign_sidecar_is_not_rewritten_by_write_sidecar() {
        let (outcome, after) = write_over("darktable", DARKTABLE.as_bytes(), &rated());
        assert_eq!(outcome, SidecarOutcome::SkippedForeign);
        assert_eq!(
            after.as_deref(),
            Some(DARKTABLE.as_bytes()),
            "another app's sidecar must be byte-identical after a Lumina write"
        );
    }

    #[test]
    fn foreign_sidecar_with_empty_metadata_is_not_deleted() {
        let (outcome, after) = write_over(
            "darktable_empty",
            DARKTABLE.as_bytes(),
            &SidecarMetadata::default(),
        );
        assert_eq!(outcome, SidecarOutcome::SkippedForeign);
        assert_eq!(
            after.as_deref(),
            Some(DARKTABLE.as_bytes()),
            "blank Lumina fields must never delete another app's sidecar"
        );
    }

    #[test]
    fn unreadable_sidecar_is_skipped() {
        let invalid = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\xff\xfe\x00</x:xmpmeta>";
        for (name, metadata) in [
            ("bad_utf8", rated()),
            ("bad_utf8_empty", SidecarMetadata::default()),
        ] {
            let (outcome, after) = write_over(name, invalid, &metadata);
            assert_eq!(outcome, SidecarOutcome::SkippedForeign, "{name}");
            assert_eq!(
                after.as_deref(),
                Some(&invalid[..]),
                "a sidecar that is not UTF-8 must be left unchanged ({name})"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn sidecar_lumina_cannot_read_is_left_alone() {
        use std::os::unix::fs::PermissionsExt;
        let media = temp_media("unreadable_perm");
        let sidecar = crate::xmp::sidecar_path(&media);
        let bytes = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><darktable:history/></x:xmpmeta>";
        fs::write(&sidecar, bytes).unwrap();
        fs::set_permissions(&sidecar, fs::Permissions::from_mode(0o000)).unwrap();
        let outcome = write_sidecar(&media, &rated()).unwrap();
        fs::set_permissions(&sidecar, fs::Permissions::from_mode(0o644)).unwrap();
        let after = fs::read(&sidecar).unwrap();
        fs::remove_dir_all(media.parent().unwrap()).unwrap();
        assert_eq!(outcome, SidecarOutcome::SkippedForeign);
        assert_eq!(
            after,
            bytes.to_vec(),
            "a sidecar Lumina cannot read may be another app's and must not be replaced"
        );
    }

    #[test]
    fn utf16_sidecar_is_skipped() {
        let lumina = render_xmp(&rated());
        for (name, bom, encode) in [
            (
                "utf16le",
                [0xFF, 0xFE],
                u16::to_le_bytes as fn(u16) -> [u8; 2],
            ),
            (
                "utf16be",
                [0xFE, 0xFF],
                u16::to_be_bytes as fn(u16) -> [u8; 2],
            ),
        ] {
            let mut bytes = bom.to_vec();
            bytes.extend(lumina.encode_utf16().flat_map(encode));
            let (outcome, after) = write_over(name, &bytes, &rated());
            assert_eq!(outcome, SidecarOutcome::SkippedForeign, "{name}");
            assert_eq!(
                after,
                Some(bytes),
                "a UTF-16 sidecar must be left unchanged ({name})"
            );
            // Without a byte order mark the NULs still read as UTF-8.
            let bare: Vec<u8> = lumina.encode_utf16().flat_map(encode).collect();
            let (outcome, after) = write_over(name, &bare, &SidecarMetadata::default());
            assert_eq!(
                outcome,
                SidecarOutcome::SkippedForeign,
                "{name} without BOM"
            );
            assert_eq!(
                after,
                Some(bare),
                "a UTF-16 sidecar without BOM is not deleted ({name})"
            );
        }
    }

    #[test]
    fn own_sidecar_is_rewritten_and_removed() {
        let lumina = render_xmp(&rated());
        let (outcome, after) = write_over(
            "own",
            lumina.as_bytes(),
            &SidecarMetadata {
                rating: 2,
                ..Default::default()
            },
        );
        assert!(matches!(outcome, SidecarOutcome::Written(_)), "{outcome:?}");
        assert!(String::from_utf8(after.unwrap())
            .unwrap()
            .contains("xmp:Rating=\"2\""));
        let (outcome, after) =
            write_over("own_empty", lumina.as_bytes(), &SidecarMetadata::default());
        assert_eq!(outcome, SidecarOutcome::Removed);
        assert_eq!(
            after, None,
            "Lumina's own sidecar is removed when its fields are blank"
        );
        let (outcome, _) = write_over("blank", b"", &rated());
        assert!(
            matches!(outcome, SidecarOutcome::Written(_)),
            "an empty file holds nothing to lose: {outcome:?}"
        );
    }
}
