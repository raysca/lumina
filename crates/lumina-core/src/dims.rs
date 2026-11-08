//! The displayed pixel size of a photo, as the catalog stores it: the size
//! after EXIF orientation, so a portrait phone photo is taller than wide.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// `size` as displayed under EXIF `orientation`: 5-8 turn the stored image a
/// quarter turn, so width and height swap. 1-4 (and unknown values) keep it.
pub(crate) fn orient(size: (u32, u32), orientation: u32) -> (u32, u32) {
    if (5..=8).contains(&orientation) {
        (size.1, size.0)
    } else {
        size
    }
}

/// What EXIF says about a photo's size: the stored PixelX/YDimension (when
/// both are present) and the orientation (1 when absent).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ExifSize {
    pub stored: Option<(u32, u32)>,
    pub orientation: u32,
}

impl Default for ExifSize {
    fn default() -> Self {
        Self {
            stored: None,
            orientation: 1,
        }
    }
}

impl ExifSize {
    /// Reads the size and orientation from the primary image's EXIF.
    /// PixelX/YDimension count the stored pixels, before orientation.
    pub(crate) fn read(exif: &exif::Exif) -> Self {
        let field = |tag| {
            exif.get_field(tag, exif::In::PRIMARY)
                .and_then(|f| f.value.get_uint(0))
        };
        Self {
            stored: field(exif::Tag::PixelXDimension).zip(field(exif::Tag::PixelYDimension)),
            orientation: field(exif::Tag::Orientation).unwrap_or(1),
        }
    }
}

/// The displayed size from the cheapest source that knows it: the decoded
/// header (`header`), else EXIF PixelX/YDimension, else macOS's own reader
/// (one `sips` call, so only files the first two cannot size pay it: HEIC
/// written without the EXIF size, TIFF and RAW without it). All three report
/// the stored size, before orientation (checked for JPEG, HEIC and TIFF; for
/// RAW it is unverified), so the EXIF orientation is applied last.
/// (0, 0) when nothing can size the file.
pub(crate) fn displayed_size(
    path: &Path,
    header: Option<(u32, u32)>,
    exif: ExifSize,
) -> (u32, u32) {
    let known = |s: &(u32, u32)| s.0 > 0 && s.1 > 0;
    header
        .filter(known)
        .or(exif.stored.filter(known))
        .or_else(|| {
            // A JPEG/PNG/WebP whose header the image crate cannot read is
            // broken; asking sips about it only costs a process.
            let has_codec = image::ImageFormat::from_path(path).is_ok_and(|f| f.reading_enabled());
            if has_codec {
                return None;
            }
            crate::decoder::native_pixel_size(path).filter(known)
        })
        .map_or((0, 0), |size| orient(size, exif.orientation))
}

/// Reads `path`'s EXIF size and orientation; the defaults when it has none.
pub(crate) fn read_exif_size(path: &Path) -> ExifSize {
    File::open(path)
        .ok()
        .and_then(|file| {
            exif::Reader::new()
                .read_from_container(&mut BufReader::new(file))
                .ok()
        })
        .map_or_else(ExifSize::default, |exif| ExifSize::read(&exif))
}

/// Reads `path` and returns its displayed size, `None` when the file cannot
/// be opened or sized. For the one-time catalog backfill.
pub(crate) fn read_displayed_size(path: &Path) -> Option<(u32, u32)> {
    let header = image::image_dimensions(path).ok();
    let size = displayed_size(path, header, read_exif_size(path));
    (size != (0, 0)).then_some(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarter_turn_orientations_swap_width_and_height() {
        for o in 1..=4 {
            assert_eq!(
                orient((40, 30), o),
                (40, 30),
                "orientation {o} keeps the size"
            );
        }
        for o in 5..=8 {
            assert_eq!(
                orient((40, 30), o),
                (30, 40),
                "orientation {o} is a quarter turn"
            );
        }
        assert_eq!(
            orient((40, 30), 0),
            (40, 30),
            "an invalid value keeps the size"
        );
    }

    #[test]
    fn scanner_swaps_dims_for_orientation_6() {
        let dir = std::env::temp_dir().join(format!(
            "lumina_dims_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("portrait.jpg");
        crate::decoder::tests::orientation_six_jpeg(&path, 8, 4);
        let (tx, _rx) = crossbeam_channel::unbounded();
        let scanned = crate::scanner::Scanner::new(dir.clone(), tx)
            .inspect_media_file(&path)
            .map(|item| (item.width, item.height));
        let read = read_displayed_size(&path);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            scanned,
            Some((4, 8)),
            "the scanner stores the displayed size of an orientation-6 photo"
        );
        assert_eq!(read, Some((4, 8)), "the backfill reads the same size");
    }
}
