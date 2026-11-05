//! The sample library offered on the welcome screen: generated gradient
//! photos, so there is no licence to track and no real place or person in them.

use std::fs::File;
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use image::{ImageFormat, Rgb, RgbImage};
use rayon::prelude::*;

/// How many photos the sample library holds.
pub const SAMPLE_COUNT: usize = 24;
const WIDTH: u32 = 1600;
const HEIGHT: u32 = 1067;
/// The photos' dates are spread over this many days before now.
const SPAN_DAYS: u64 = 90;

/// Writes the missing `sample_NN.jpg` gradients into `dest` (created if
/// needed) and dates them over the last 90 days, newest first. Files already
/// there are kept as they are, so a second call writes nothing. Returns how
/// many files were written.
pub fn write_sample_library(dest: &Path) -> anyhow::Result<usize> {
    std::fs::create_dir_all(dest)
        .with_context(|| format!("Could not create {}", dest.display()))?;
    let now = SystemTime::now();
    let written: Vec<bool> = (0..SAMPLE_COUNT)
        .into_par_iter()
        .map(|i| write_one(dest, i, now))
        .collect::<anyhow::Result<_>>()?;
    Ok(written.into_iter().filter(|w| *w).count())
}

/// Writes sample `i` unless it exists. The JPEG goes to a temporary name
/// first, so an interrupted write never leaves a truncated photo behind that
/// a later call would keep.
fn write_one(dest: &Path, i: usize, now: SystemTime) -> anyhow::Result<bool> {
    let path = dest.join(format!("sample_{:02}.jpg", i + 1));
    if path.exists() {
        return Ok(false);
    }
    let partial = dest.join(format!(".sample_{:02}.jpg.part", i + 1));
    gradient(i)
        .save_with_format(&partial, ImageFormat::Jpeg)
        .with_context(|| format!("Could not write {}", partial.display()))?;
    let age = Duration::from_secs(SPAN_DAYS * 86_400 * i as u64 / (SAMPLE_COUNT as u64 - 1));
    File::options()
        .write(true)
        .open(&partial)
        .and_then(|file| file.set_modified(now - age))
        .with_context(|| format!("Could not date {}", partial.display()))?;
    std::fs::rename(&partial, &path)
        .with_context(|| format!("Could not write {}", path.display()))?;
    Ok(true)
}

/// A diagonal two-colour gradient; the hue turns a little for each sample.
fn gradient(i: usize) -> RgbImage {
    let hue = i as f32 / SAMPLE_COUNT as f32;
    let from = hue_to_rgb(hue);
    let to = hue_to_rgb((hue + 0.3) % 1.0);
    let span = (WIDTH + HEIGHT) as f32;
    RgbImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let t = (x + y) as f32 / span;
        Rgb(std::array::from_fn(|c| {
            (from[c] + (to[c] - from[c]) * t).round() as u8
        }))
    })
}

/// A saturated, fairly light colour for `hue` in 0..1 (HSV, s 0.6, v 0.9).
fn hue_to_rgb(hue: f32) -> [f32; 3] {
    let (s, v) = (0.6, 0.9);
    let h = hue * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [(r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_library_writes_decodable_jpegs_idempotently() {
        let dest = std::env::temp_dir().join(format!(
            "lumina_sample_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        // A file already there must survive untouched.
        std::fs::create_dir_all(&dest).unwrap();
        let kept = dest.join("sample_01.jpg");
        std::fs::write(&kept, b"the user's own file").unwrap();

        let written = write_sample_library(&dest).expect("first run writes the samples");
        assert_eq!(
            written,
            SAMPLE_COUNT - 1,
            "only the missing samples are written"
        );
        assert_eq!(
            std::fs::read(&kept).unwrap(),
            b"the user's own file",
            "an existing file must not be rewritten"
        );
        std::fs::remove_file(&kept).unwrap();
        assert_eq!(
            write_sample_library(&dest).unwrap(),
            1,
            "a missing sample is restored"
        );
        let before: Vec<_> = (1..=SAMPLE_COUNT)
            .map(|n| {
                std::fs::metadata(dest.join(format!("sample_{n:02}.jpg")))
                    .unwrap()
                    .modified()
                    .unwrap()
            })
            .collect();
        assert_eq!(
            write_sample_library(&dest).unwrap(),
            0,
            "a second run writes nothing"
        );

        let mut files: Vec<_> = std::fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        assert_eq!(
            files.len(),
            SAMPLE_COUNT,
            "exactly the samples, no partial files"
        );
        let mut mtimes = Vec::new();
        for path in &files {
            let img = image::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!((img.width(), img.height()), (WIDTH, HEIGHT));
            mtimes.push(std::fs::metadata(path).unwrap().modified().unwrap());
        }
        assert_eq!(mtimes, before, "the second run must not touch any file");
        let span = mtimes
            .iter()
            .max()
            .unwrap()
            .duration_since(*mtimes.iter().min().unwrap())
            .unwrap();
        assert!(
            span >= Duration::from_secs(60 * 86_400),
            "the samples' dates span at least two months, got {span:?}"
        );
        std::fs::remove_dir_all(&dest).unwrap();
    }
}
