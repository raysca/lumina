//! Cleaning for user-entered text before it reaches the catalog or an XMP
//! sidecar. Every field is one line, so the rules are the same everywhere;
//! only the length cap differs.

/// Longest tag or smart album name, in characters.
pub const MAX_NAME_CHARS: usize = 100;
/// Longest title, caption or search text, in characters.
pub const MAX_TEXT_CHARS: usize = 2000;

/// True for characters a field never keeps: control characters, line and
/// paragraph separators, bidi overrides (they can disguise what a name
/// says), the byte-order mark, and noncharacters, which are invalid in XML
/// and would make a sidecar unreadable. Zero-width joiners stay, since emoji
/// sequences need them.
pub fn is_disallowed(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{2028}' | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FDD0}'..='\u{FDEF}'
        )
        || (c as u32) & 0xFFFE == 0xFFFE
}

/// `raw` as one clean line: disallowed characters and tabs or line breaks
/// become spaces, runs of whitespace collapse to one space, the ends are
/// trimmed, and the result is cut to `max_chars` characters.
/// ponytail: no Unicode normalisation (NFC), which needs a new dependency;
/// add it if decomposed names start showing up as duplicates.
pub fn clean_text(raw: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(raw.len().min(max_chars * 4));
    let mut count = 0;
    let mut pending_space = false;
    for c in raw.chars() {
        if c.is_whitespace() || is_disallowed(c) {
            pending_space = count > 0;
            continue;
        }
        if pending_space {
            if count + 1 >= max_chars {
                break;
            }
            out.push(' ');
            count += 1;
            pending_space = false;
        }
        if count >= max_chars {
            break;
        }
        out.push(c);
        count += 1;
    }
    out
}

/// Like [`clean_text`] but keeps line breaks, since captions imported from
/// other apps often have paragraphs. Each line is cleaned on its own.
pub fn clean_caption(raw: &str, max_chars: usize) -> String {
    let lines: Vec<String> = raw
        .lines()
        .map(|line| clean_text(line, max_chars))
        .collect();
    let capped: String = lines
        .join("\n")
        .trim_matches('\n')
        .chars()
        .take(max_chars)
        .collect();
    capped.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_clean_line() {
        assert_eq!(
            clean_text("  Summer\t\n  in Lisbon \r", 100),
            "Summer in Lisbon"
        );
        assert_eq!(
            clean_text("a\u{0}b\u{7}c", 100),
            "a b c",
            "controls become spaces"
        );
        assert_eq!(
            clean_text("\u{2028}x\u{2029}", 100),
            "x",
            "line separators go"
        );
        assert_eq!(clean_text("", 100), "");
        assert_eq!(clean_text(" \t\n ", 100), "", "blank stays blank");
    }

    #[test]
    fn drops_characters_that_disguise_or_corrupt() {
        assert_eq!(
            clean_text("gpj.\u{202E}exe", 100),
            "gpj. exe",
            "bidi override"
        );
        assert_eq!(clean_text("\u{FEFF}tag", 100), "tag", "byte-order mark");
        assert_eq!(
            clean_text("a\u{FFFE}b\u{FFFF}c\u{1FFFF}", 100),
            "a b c",
            "noncharacters are invalid in XML sidecars"
        );
        let family = "👨\u{200D}👩\u{200D}👧";
        assert_eq!(clean_text(family, 100), family, "emoji joiners stay");
        assert_eq!(clean_text("café 東京", 100), "café 東京");
    }

    #[test]
    fn a_caption_keeps_its_paragraphs() {
        assert_eq!(
            clean_caption("\n First\u{202E} line \r\n\nSecond\n\n", 100),
            "First line\n\nSecond"
        );
    }

    #[test]
    fn caps_length_on_a_character_boundary() {
        assert_eq!(clean_text("abcdef", 3), "abc");
        assert_eq!(clean_text("ééééé", 2), "éé", "counts characters, not bytes");
        assert_eq!(clean_text("ab cd", 3), "ab", "never ends on a space");
        assert_eq!(
            clean_text(&"x".repeat(5000), MAX_TEXT_CHARS)
                .chars()
                .count(),
            2000
        );
    }
}
