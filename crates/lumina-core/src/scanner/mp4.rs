//! Video duration and size from MP4/MOV headers (`mvhd`, `tkhd`), with a
//! Spotlight fallback for the duration.

use std::path::Path;

fn parse_mvhd_duration(data: &[u8]) -> Option<(u64, f64)> {
    let pos = data.windows(4).position(|w| w == b"mvhd")?;
    let v_pos = pos + 4;
    if v_pos >= data.len() {
        return None;
    }
    let version = data[v_pos];
    if version == 0 && pos + 24 <= data.len() {
        let ts = u32::from_be_bytes([
            data[pos + 16],
            data[pos + 17],
            data[pos + 18],
            data[pos + 19],
        ]) as u64;
        let dur = u32::from_be_bytes([
            data[pos + 20],
            data[pos + 21],
            data[pos + 22],
            data[pos + 23],
        ]) as u64;
        if ts > 0 {
            let secs = (dur as f64) / (ts as f64);
            let ms = ((dur as u128 * 1000) / ts as u128).min(u64::MAX as u128) as u64;
            return Some((ms, secs));
        }
    } else if version == 1 && pos + 36 <= data.len() {
        let ts = u32::from_be_bytes([
            data[pos + 24],
            data[pos + 25],
            data[pos + 26],
            data[pos + 27],
        ]) as u64;
        let dur = u64::from_be_bytes([
            data[pos + 28],
            data[pos + 29],
            data[pos + 30],
            data[pos + 31],
            data[pos + 32],
            data[pos + 33],
            data[pos + 34],
            data[pos + 35],
        ]);
        if ts > 0 {
            let secs = (dur as f64) / (ts as f64);
            let ms = ((dur as u128 * 1000) / ts as u128).min(u64::MAX as u128) as u64;
            return Some((ms, secs));
        }
    }
    None
}

/// Seconds from the QuickTime epoch (1904-01-01) to the Unix epoch.
const QUICKTIME_EPOCH_OFFSET_SECS: u64 = 2_082_844_800;

/// The `mvhd` creation time in Unix ms. Zero (never set) and anything before
/// 1970 (a camera with no clock) count as unknown.
fn parse_mvhd_created(data: &[u8]) -> Option<i64> {
    let pos = data.windows(4).position(|w| w == b"mvhd")?;
    let secs = match *data.get(pos + 4)? {
        0 => u32::from_be_bytes(data.get(pos + 8..pos + 12)?.try_into().ok()?) as u64,
        1 => u64::from_be_bytes(data.get(pos + 8..pos + 16)?.try_into().ok()?),
        _ => return None,
    };
    let unix = secs.checked_sub(QUICKTIME_EPOCH_OFFSET_SECS)?;
    let ms = i64::try_from(unix).ok()?.checked_mul(1000)?;
    (ms > 0 && chrono::DateTime::from_timestamp_millis(ms).is_some()).then_some(ms)
}

/// Displayed size of the first visual track, from its `tkhd` box. Audio
/// tracks have a zero size and are skipped. A 90°/270° matrix (portrait phone
/// video stored landscape) swaps width and height.
fn parse_tkhd_size(data: &[u8]) -> Option<(u32, u32)> {
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let mut from = 0;
    while let Some(found) = data[from..].windows(4).position(|w| w == b"tkhd") {
        let pos = from + found;
        from = pos + 4;
        // A real box header carries the fixed tkhd size; random video bytes
        // that happen to spell "tkhd" do not.
        if !matches!(pos.checked_sub(4).and_then(be32), Some(92 | 104)) {
            continue;
        }
        // version + flags, then times/ids (20 bytes in v0, 32 in v1), then
        // 16 reserved/layer/volume bytes and the 36-byte matrix.
        let matrix = pos
            + 4
            + 4
            + if data.get(pos + 4) == Some(&1) {
                32
            } else {
                20
            }
            + 16;
        let (a, b) = (be32(matrix)? as i32, be32(matrix + 4)? as i32);
        let (w, h) = (be32(matrix + 36)? >> 16, be32(matrix + 40)? >> 16);
        if w > 0 && h > 0 {
            return Some(if a == 0 && b != 0 { (h, w) } else { (w, h) });
        }
    }
    None
}

/// Runs `parse` over the head of a movie file, then over its tail, where
/// non-faststart MP4/MOV files keep the `moov` atom.
fn scan_movie_header<T>(path: &Path, parse: impl Fn(&[u8]) -> Option<T>) -> Option<T> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = vec![0u8; 128 * 1024]; // 128 KiB head check
    if let Ok(n) = file.read(&mut head) {
        if let Some(res) = parse(&head[..n]) {
            return Some(res);
        }
    }
    let file_len = file.metadata().ok()?.len();
    if file_len > 128 * 1024 {
        file.seek(SeekFrom::Start(file_len.saturating_sub(256 * 1024)))
            .ok()?;
        let mut tail = Vec::with_capacity(256 * 1024);
        file.read_to_end(&mut tail).ok()?;
        return parse(&tail);
    }
    None
}

/// Displayed `(width, height)` of an MP4/MOV video, read from its headers.
pub fn extract_video_size(path: &Path) -> Option<(u32, u32)> {
    scan_movie_header(path, parse_tkhd_size)
}

/// When an MP4/MOV video was recorded (Unix ms, UTC), from its `mvhd`
/// creation time; `None` when the header has none.
pub fn extract_video_created(path: &Path) -> Option<i64> {
    scan_movie_header(path, parse_mvhd_created)
}

/// Duration of a video as `(ms, seconds)`: from MP4/MOV headers, else from
/// Spotlight on macOS (bounded by the native tool timeout). `None` when
/// neither knows.
pub fn extract_video_duration(path: &Path) -> Option<(u64, f64)> {
    if let Some(res) = scan_movie_header(path, parse_mvhd_duration) {
        return Some(res);
    }

    spotlight_duration(path)
}

/// Spotlight's value, for containers the header parser can't read (MKV,
/// AVI, WebM). Empty until the file is indexed. The output is one short
/// line, far below a pipe buffer, so reading it after the exit cannot
/// deadlock.
#[cfg(target_os = "macos")]
fn spotlight_duration(path: &Path) -> Option<(u64, f64)> {
    use std::io::Read;
    let mut child = std::process::Command::new("mdls")
        .arg("-name")
        .arg("kMDItemDurationSeconds")
        .arg(path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    if !crate::decoder::wait_with_timeout(child, crate::decoder::NATIVE_TOOL_TIMEOUT)
        .ok()?
        .success()
    {
        return None;
    }
    let mut text = String::new();
    stdout.read_to_string(&mut text).ok()?;
    let secs = text.split('=').nth(1)?.trim().parse::<f64>().ok()?;
    (secs > 0.0).then_some(((secs * 1000.0) as u64, secs))
}

/// Spotlight is macOS only: elsewhere a scan must not run whatever `mdls`
/// happens to be on the PATH for every video the header parser can't read.
#[cfg(not(target_os = "macos"))]
fn spotlight_duration(_path: &Path) -> Option<(u64, f64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mvhd_creation_time_is_read_in_both_versions_and_unset_is_none() {
        // 2021-06-01T12:00:00Z
        let unix: u64 = 1_622_548_800;
        let mut v0 = vec![0u8; 32];
        v0[0..4].copy_from_slice(b"mvhd");
        v0[8..12].copy_from_slice(&((unix + QUICKTIME_EPOCH_OFFSET_SECS) as u32).to_be_bytes());
        assert_eq!(
            parse_mvhd_created(&v0),
            Some(unix as i64 * 1000),
            "v0 is a u32"
        );

        let mut v1 = vec![0u8; 40];
        v1[0..4].copy_from_slice(b"mvhd");
        v1[4] = 1;
        v1[8..16].copy_from_slice(&(unix + QUICKTIME_EPOCH_OFFSET_SECS).to_be_bytes());
        assert_eq!(
            parse_mvhd_created(&v1),
            Some(unix as i64 * 1000),
            "v1 is a u64"
        );

        let mut unset = vec![0u8; 32];
        unset[0..4].copy_from_slice(b"mvhd");
        assert_eq!(
            parse_mvhd_created(&unset),
            None,
            "0 means the camera set none"
        );
        unset[8..12].copy_from_slice(&1000u32.to_be_bytes());
        assert_eq!(
            parse_mvhd_created(&unset),
            None,
            "1904 is a clockless camera"
        );
        assert_eq!(
            parse_mvhd_created(&v0[..10]),
            None,
            "a truncated box is unknown"
        );
    }

    #[test]
    fn parse_mvhd_duration_v1_large_duration_does_not_overflow() {
        let mut data = vec![0u8; 64];
        data[0..4].copy_from_slice(b"mvhd");
        data[4] = 1; // version 1
        let ts: u32 = 1000;
        data[24..28].copy_from_slice(&ts.to_be_bytes());
        let dur: u64 = u64::MAX / 500;
        data[28..36].copy_from_slice(&dur.to_be_bytes());

        let res = parse_mvhd_duration(&data);
        assert!(res.is_some());
        let (ms, _secs) = res.unwrap();
        assert_eq!(ms, dur);
    }
}
