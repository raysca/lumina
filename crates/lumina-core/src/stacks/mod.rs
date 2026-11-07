//! Stacks: RAW+JPEG pairs and bursts, computed on demand for a review
//! view. Nothing is stored; exact copies stay in `duplicates`, and stacks
//! never read `media_hashes`.

use anyhow::Result;
use std::collections::HashMap;

use crate::catalog::files::PRESENT;
use crate::catalog::media_type_from_db;
use crate::{Catalog, LuminaEngine, MediaId, MediaType};

/// Why a set of items forms a stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackKind {
    /// One RAW and one image with the same folder and file stem.
    RawJpeg,
    /// Three or more stills from one camera, each within 1 s of the last.
    Burst,
}

/// A group of items to review together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    /// Why these items were grouped.
    pub kind: StackKind,
    /// Members in a fixed order: `[image, raw]` for a pair (the image
    /// represents it), time order (then id) for a burst. "The first" item
    /// is the image of a pair and the earliest shot of a burst.
    pub items: Vec<MediaId>,
}

/// Largest gap between consecutive burst shots. Timestamps are whole
/// seconds, so this means "same or next second".
const BURST_GAP_MS: i64 = 1000;
const BURST_MIN_ITEMS: usize = 3;

struct Row {
    id: MediaId,
    parent_dir: String,
    file_name: String,
    /// Trimmed make and model, both non-blank, or `None`.
    camera: Option<(String, String)>,
    ts: i64,
    media_type: MediaType,
}

impl Catalog {
    /// See [`LuminaEngine::find_stacks`].
    pub fn find_stacks(&self) -> Result<Vec<Stack>> {
        Ok(group(self.stack_rows()?))
    }

    /// Active, present rows in `(timestamp, id)` order, without edit copies.
    /// The lock is held for this read only; grouping runs on the copies.
    fn stack_rows(&self) -> Result<Vec<Row>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare_cached(&stack_rows_sql())?;
        let rows = stmt.query_map([], |row| {
            let make: Option<String> = row.get(3)?;
            let model: Option<String> = row.get(4)?;
            let media_type: String = row.get(6)?;
            Ok(Row {
                id: MediaId(row.get::<_, i64>(0)? as u64),
                parent_dir: row.get(1)?,
                file_name: row.get(2)?,
                camera: camera(make.as_deref(), model.as_deref()),
                ts: row.get(5)?,
                media_type: media_type_from_db(&media_type),
            })
        })?;
        let mut rows: Vec<Row> = rows.collect::<rusqlite::Result<_>>()?;
        rows.retain(|r| !is_edit_copy_name(&r.file_name));
        Ok(rows)
    }
}

impl LuminaEngine {
    /// Every RAW+JPEG pair and burst. Reads the catalog once; call it off
    /// the UI thread. A wrong stack is worse than a missed one (the review
    /// view rejects members), so anything doubtful is left out.
    ///
    /// Never stacked: trashed, hidden and missing items, and edit copies
    /// (`name (edited).ext`, see [`LuminaEngine::duplicate_for_edit`]).
    ///
    /// - **RawJpeg**: same `parent_dir` and file stem (case-insensitive),
    ///   exactly one RAW plus one image. Any other stem group (two JPEGs
    ///   and a RAW, `IMG_1.JPG` + `img_1.jpg`, a video + image) is skipped,
    ///   never guessed. Both members need the same known (non-blank) make
    ///   and model; a missing camera on either side never pairs, so RAWs
    ///   the EXIF reader cannot parse (likely CR3, RAF) are never paired.
    ///   No time check: a RAW may carry only its mtime.
    /// - **Burst**: >= 3 stills (image or RAW) in one folder with the same
    ///   non-blank make and model, each within 1000 ms of the previous.
    ///   Folders never mix (a backup copy of an import is not a burst). A
    ///   paired RAW is left out, so a pair counts once, as its image; the
    ///   pair is still listed as its own RawJpeg stack.
    ///
    /// Order: by the earliest timestamp in the stack, then RawJpeg before
    /// Burst, then the first item's id. See [`Stack::items`] for members.
    pub fn find_stacks(&self) -> Result<Vec<Stack>> {
        self.catalog().find_stacks()
    }
}

fn stack_rows_sql() -> String {
    format!(
        "SELECT m.id, m.parent_dir, m.file_name, m.make, m.model, \
         m.timestamp_epoch_ms, m.media_type FROM media_items m \
         WHERE m.is_trashed = 0 AND m.is_hidden = 0 AND {PRESENT} \
         ORDER BY m.timestamp_epoch_ms ASC, m.id ASC;"
    )
}

fn camera(make: Option<&str>, model: Option<&str>) -> Option<(String, String)> {
    let (make, model) = (make?.trim(), model?.trim());
    (!make.is_empty() && !model.is_empty()).then(|| (make.to_string(), model.to_string()))
}

/// Whether `file_name` is a name [`LuminaEngine::duplicate_for_edit`] picks:
/// `stem (edited).ext` or `stem (edited) (n).ext`.
/// ponytail: name only, since the catalog keeps no copy marker. A renamed
/// copy, or a Finder "copy" duplicate, is not detected; a durable marker
/// (a column or relation written by `insert_edit_copy`) is the upgrade.
fn is_edit_copy_name(file_name: &str) -> bool {
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let stem = match stem.strip_suffix(')').and_then(|s| s.rsplit_once(" (")) {
        Some((head, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => stem,
    };
    stem.ends_with(" (edited)")
}

/// `rows` must be in `(timestamp, id)` order.
fn group(rows: Vec<Row>) -> Vec<Stack> {
    let is_raw = |r: &Row| r.media_type == MediaType::Raw;
    let is_image = |r: &Row| r.media_type == MediaType::Image;

    // RAW+JPEG: bucket by (dir, lowercase stem), keep exact 1 RAW + 1 image
    // with the same known camera.
    let mut by_stem: HashMap<(&str, String), Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let stem = r
            .file_name
            .rsplit_once('.')
            .map_or(r.file_name.as_str(), |(s, _)| s);
        by_stem
            .entry((r.parent_dir.as_str(), stem.to_lowercase()))
            .or_default()
            .push(i);
    }
    let mut paired_raw = vec![false; rows.len()];
    let mut keyed: Vec<((i64, u8, u64), Stack)> = Vec::new();
    for members in by_stem.into_values() {
        let [a, b] = members[..] else { continue };
        let (img, raw) = if is_image(&rows[a]) && is_raw(&rows[b]) {
            (a, b)
        } else if is_raw(&rows[a]) && is_image(&rows[b]) {
            (b, a)
        } else {
            continue;
        };
        // ponytail: both cameras must be known and equal, so RAW formats the
        // EXIF reader cannot parse (likely CR3, RAF) never pair: Canon and
        // iPhone both name files IMG_NNNN. Upgrade: camera metadata from a
        // reader that handles those formats.
        if rows[img].camera.is_none() || rows[img].camera != rows[raw].camera {
            continue;
        }
        paired_raw[raw] = true;
        let ts = rows[img].ts.min(rows[raw].ts);
        keyed.push((
            (ts, 0, rows[img].id.0),
            Stack {
                kind: StackKind::RawJpeg,
                items: vec![rows[img].id, rows[raw].id],
            },
        ));
    }

    // Bursts: per folder and camera, split the time-ordered stills at gaps
    // over 1 s.
    let mut by_camera: HashMap<(&str, &str, &str), Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        if paired_raw[i] || !(is_image(r) || is_raw(r)) {
            continue;
        }
        let Some((make, model)) = &r.camera else {
            continue;
        };
        by_camera
            .entry((r.parent_dir.as_str(), make.as_str(), model.as_str()))
            .or_default()
            .push(i);
    }
    for shots in by_camera.into_values() {
        let mut flush = |run: &[usize]| {
            if run.len() >= BURST_MIN_ITEMS {
                keyed.push((
                    (rows[run[0]].ts, 1, rows[run[0]].id.0),
                    Stack {
                        kind: StackKind::Burst,
                        items: run.iter().map(|&i| rows[i].id).collect(),
                    },
                ));
            }
        };
        let mut start = 0;
        for k in 1..shots.len() {
            if rows[shots[k]].ts - rows[shots[k - 1]].ts > BURST_GAP_MS {
                flush(&shots[start..k]);
                start = k;
            }
        }
        flush(&shots[start..]);
    }

    keyed.sort_unstable_by_key(|(key, _)| *key);
    keyed.into_iter().map(|(_, stack)| stack).collect()
}

#[cfg(test)]
mod tests;
