//! Capture time fallback: embedded dates first, then file dates, never the
//! time the file was catalogued.

use super::*;
use std::time::{Duration, SystemTime};

/// 2019-07-14 09:30:00, as EXIF writes it and as the catalog stores it.
const SHOT: &str = "2019:07:14 09:30:00";
const SHOT_MS: i64 = 1_563_096_600_000;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "lumina_capture_{name}_{}_{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A big-endian TIFF whose EXIF IFD holds these ASCII date tags.
fn exif_dates(tags: &[(u16, &str)]) -> Vec<u8> {
    let exif_ifd = 26u32;
    let mut data_at = exif_ifd + 2 + 12 * tags.len() as u32 + 4;
    let mut tiff = b"MM\0\x2a\0\0\0\x08\0\x01".to_vec();
    tiff.extend_from_slice(&[0x87, 0x69, 0, 4, 0, 0, 0, 1]);
    tiff.extend_from_slice(&exif_ifd.to_be_bytes());
    tiff.extend_from_slice(&[0; 4]);
    tiff.extend_from_slice(&(tags.len() as u16).to_be_bytes());
    for (tag, _) in tags {
        tiff.extend_from_slice(&tag.to_be_bytes());
        tiff.extend_from_slice(&[0, 2, 0, 0, 0, 20]);
        tiff.extend_from_slice(&data_at.to_be_bytes());
        data_at += 20;
    }
    tiff.extend_from_slice(&[0; 4]);
    for (_, value) in tags {
        tiff.extend_from_slice(value.as_bytes());
        tiff.push(0);
    }
    tiff
}

fn write_jpeg(path: &Path, tiff: Option<&[u8]>) {
    let image = image::RgbImage::from_pixel(4, 4, image::Rgb([90, 90, 90]));
    let bytes = match tiff {
        Some(tiff) => crate::decoder::tests::jpeg_with_exif(&image, tiff),
        None => {
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
                .encode_image(&image)
                .expect("fixture encodes");
            jpeg
        }
    };
    std::fs::write(path, bytes).expect("fixture writes");
}

/// A minimal MOV: an `mvhd` (v0) with this creation time in Unix seconds
/// (0 for unset) and a one-second duration.
fn write_mov(path: &Path, created_unix: u32) {
    let created = if created_unix == 0 {
        0
    } else {
        created_unix + 2_082_844_800
    };
    let mut mvhd = 108u32.to_be_bytes().to_vec();
    mvhd.extend_from_slice(b"mvhd\0\0\0\0");
    mvhd.extend_from_slice(&created.to_be_bytes());
    mvhd.extend_from_slice(&created.to_be_bytes());
    mvhd.extend_from_slice(&1000u32.to_be_bytes());
    mvhd.extend_from_slice(&1000u32.to_be_bytes());
    mvhd.resize(108, 0);
    let mut moov = (8 + mvhd.len() as u32).to_be_bytes().to_vec();
    moov.extend_from_slice(b"moov");
    moov.extend_from_slice(&mvhd);
    std::fs::write(path, moov).expect("fixture writes");
}

fn set_mtime(path: &Path, at: SystemTime) {
    File::options()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(at))
        .expect("set mtime");
}

fn ms(at: SystemTime) -> i64 {
    at.duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

/// The capture time the scanner stores, checked against the backfill's.
fn stored_capture_time(dir: &Path, path: &Path) -> i64 {
    let (tx, _rx) = crossbeam_channel::unbounded();
    let item = Scanner::new(dir.to_path_buf(), tx)
        .inspect_media_file(path)
        .expect("the scanner reads the fixture");
    assert_eq!(
        capture_time(path),
        Some(item.timestamp_epoch_ms),
        "the backfill computes the same date as the scanner"
    );
    item.timestamp_epoch_ms
}

#[test]
fn exif_dates_win_in_order_original_then_digitized() {
    let dir = TempDir::new("exif");
    let digitized = dir.0.join("digitized.jpg");
    write_jpeg(&digitized, Some(&exif_dates(&[(0x9004, SHOT)])));
    assert_eq!(
        stored_capture_time(&dir.0, &digitized),
        SHOT_MS,
        "DateTimeDigitized (CreateDate) dates a photo with no DateTimeOriginal"
    );

    let both = dir.0.join("both.jpg");
    write_jpeg(
        &both,
        Some(&exif_dates(&[
            (0x9003, "2020:01:02 03:04:05"),
            (0x9004, SHOT),
        ])),
    );
    assert_eq!(
        stored_capture_time(&dir.0, &both),
        1_577_934_245_000,
        "DateTimeOriginal wins over DateTimeDigitized"
    );
}

#[test]
fn video_is_dated_by_its_quicktime_creation_time() {
    let dir = TempDir::new("video");
    let clip = dir.0.join("clip.mov");
    write_mov(&clip, (SHOT_MS / 1000) as u32);
    assert_eq!(
        stored_capture_time(&dir.0, &clip),
        SHOT_MS,
        "a video takes its mvhd creation time, not its file dates"
    );

    let unset = dir.0.join("unset.mov");
    write_mov(&unset, 0);
    let old = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    set_mtime(&unset, old);
    assert_eq!(
        stored_capture_time(&dir.0, &unset),
        ms(old),
        "a video with no creation time falls back to its file dates"
    );
}

#[test]
fn no_embedded_date_uses_the_older_of_birth_and_modification_time() {
    let dir = TempDir::new("file");
    // Copied with its original mtime kept: born now, modified long ago.
    let copied = dir.0.join("copied.jpg");
    write_jpeg(&copied, None);
    let old = UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    set_mtime(&copied, old);
    assert_eq!(
        stored_capture_time(&dir.0, &copied),
        ms(old),
        "an mtime older than the birth time wins"
    );

    // Modified after it was created: the birth time is nearer the shot.
    let touched = dir.0.join("touched.jpg");
    write_jpeg(&touched, None);
    let later = SystemTime::now() + Duration::from_secs(86_400);
    set_mtime(&touched, later);
    let birth = std::fs::metadata(&touched)
        .and_then(|m| m.created())
        .map_or(ms(later), ms);
    assert_eq!(
        stored_capture_time(&dir.0, &touched),
        birth,
        "a birth time older than the mtime wins (mtime when birth is unknown)"
    );
    assert!(
        birth < ms(later) || cfg!(not(target_os = "macos")),
        "macOS reports a birth time"
    );
}

#[test]
fn file_time_takes_the_older_of_birth_and_mtime_and_keeps_an_mtime_of_zero() {
    assert_eq!(
        older_file_time(Some(100), 200),
        100,
        "an older birth time wins"
    );
    assert_eq!(
        older_file_time(Some(300), 200),
        200,
        "a later birth time loses"
    );
    assert_eq!(older_file_time(None, 200), 200, "no birth time: the mtime");
    assert_eq!(
        older_file_time(Some(0), 200),
        200,
        "a birth time of 0 is unknown"
    );
    assert_eq!(
        older_file_time(Some(300), 0),
        0,
        "an mtime of 0 stays undated even when the birth time is known (Linux)"
    );
}
