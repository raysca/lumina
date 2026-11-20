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
mod tests;
