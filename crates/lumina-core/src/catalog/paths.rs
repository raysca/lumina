//! Path text for the catalog.
//!
//! SQLite columns hold text, but a Unix path is bytes and may not be UTF-8.
//! A path is therefore stored as its own bytes with `%` escaped and every
//! invalid UTF-8 byte written as `%XX`, so it round-trips exactly while normal
//! paths stay readable and keep working with prefix queries. `parent_dir` and
//! `file_name` remain display strings; `file_path` and `rel_path` are encoded.

use std::path::{Path, PathBuf};

/// The catalog's spelling of `path`.
pub(crate) fn encode(path: &Path) -> String {
    let mut out = String::new();
    let bytes = os_bytes(path);
    let mut rest = bytes;
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(valid) => {
                push_escaped(valid, &mut out);
                break;
            }
            Err(err) => {
                let valid_len = err.valid_up_to();
                let (valid, after) = rest.split_at(valid_len);
                push_escaped(
                    std::str::from_utf8(valid).expect("valid_up_to is valid UTF-8"),
                    &mut out,
                );
                let invalid_len = err.error_len().unwrap_or(after.len());
                for byte in &after[..invalid_len] {
                    out.push('%');
                    out.push_str(&format!("{byte:02X}"));
                }
                rest = &after[invalid_len..];
            }
        }
    }
    out
}

/// The path `stored` stands for.
pub(crate) fn decode(stored: &str) -> PathBuf {
    let bytes = stored.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi << 4 | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    path_from_bytes(out)
}

fn push_escaped(text: &str, out: &mut String) {
    for ch in text.chars() {
        if ch == '%' {
            out.push_str("%25");
        } else {
            out.push(ch);
        }
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(unix)]
fn os_bytes(path: &Path) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_encoded_bytes()
}

#[cfg(unix)]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(std::ffi::OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: Vec<u8>) -> PathBuf {
    match String::from_utf8(bytes) {
        Ok(text) => PathBuf::from(text),
        Err(err) => PathBuf::from(String::from_utf8_lossy(err.as_bytes()).into_owned()),
    }
}

/// One-shot migration: re-encode path columns written by builds that stored
/// raw, lossy spellings. A name like `100%41.jpg` must not decode as
/// `100A.jpg`; display columns (`parent_dir`, `rel_parent`) are left alone.
pub(crate) fn reencode_catalog_paths(tx: &rusqlite::Transaction<'_>) -> anyhow::Result<()> {
    for (table, columns) in [
        ("media_items", &["file_path", "rel_path"][..]),
        (
            "library_roots",
            &["mount_point", "rel_from_volume", "stored_root"][..],
        ),
    ] {
        // One read per table, then only the rows that change are written.
        let select = format!("SELECT id, {} FROM {table};", columns.join(", "));
        let rows: Vec<(i64, Vec<Option<String>>)> = {
            let mut stmt = tx.prepare(&select)?;
            let mapped = stmt.query_map([], |row| {
                let id: i64 = row.get(0)?;
                let mut values = Vec::with_capacity(columns.len());
                for index in 0..columns.len() {
                    values.push(row.get::<_, Option<String>>(index + 1)?);
                }
                Ok((id, values))
            })?;
            mapped.collect::<rusqlite::Result<_>>()?
        };
        for (id, values) in rows {
            for (column, raw) in columns.iter().zip(values) {
                let Some(raw) = raw else {
                    continue;
                };
                let encoded = encode(Path::new(&raw));
                if encoded != raw {
                    tx.execute(
                        &format!("UPDATE {table} SET {column} = ?1 WHERE id = ?2;"),
                        rusqlite::params![encoded, id],
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_paths_keep_their_spelling_and_escape_percent() {
        assert_eq!(
            encode(Path::new("/photos/2024/a b.jpg")),
            "/photos/2024/a b.jpg"
        );
        assert_eq!(
            decode("/photos/2024/a b.jpg"),
            Path::new("/photos/2024/a b.jpg")
        );
        assert_eq!(encode(Path::new("/photos/100%.jpg")), "/photos/100%25.jpg");
        assert_eq!(decode("/photos/100%25.jpg"), Path::new("/photos/100%.jpg"));
    }

    #[test]
    fn incomplete_or_unusual_percents_round_trip() {
        for path in ["/x/%", "/x/%A", "/x/%zz", "/x/é%41"] {
            let path = Path::new(path);
            assert_eq!(decode(&encode(path)), path, "{path:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_bytes_round_trip_exactly() {
        use std::os::unix::ffi::OsStrExt;
        let original = Path::new(std::ffi::OsStr::from_bytes(b"/photos/a\xff\xfe.jpg"));
        let stored = encode(original);
        assert!(
            stored.starts_with("/photos/a%FF%FE"),
            "invalid bytes are escaped: {stored}"
        );
        let back = decode(&stored);
        assert_eq!(back.as_os_str().as_bytes(), original.as_os_str().as_bytes());
    }

    #[cfg(unix)]
    #[test]
    fn a_literal_percent_is_not_mistaken_for_an_escape() {
        use std::os::unix::ffi::OsStrExt;
        let original = Path::new(std::ffi::OsStr::from_bytes(b"/photos/100%41.jpg"));
        let stored = encode(original);
        let back = decode(&stored);
        assert_eq!(
            back.as_os_str().as_bytes(),
            original.as_os_str().as_bytes(),
            "a name that merely looks like an escape still round-trips"
        );
    }
}
