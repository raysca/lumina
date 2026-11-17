//! Exporting catalogued media to a folder the user picked.

use crate::catalog::Edits;
use crate::{
    edits, xmp, DecodedFrame, DecoderPipeline, LuminaEngine, MediaId, MediaItem, MediaType,
    PixelFormat, SidecarOutcome,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Long side of a [`ExportPreset::Jpeg2048`] file.
const JPEG_LONG_SIDE: u32 = 2048;
/// JPEG quality of a [`ExportPreset::Jpeg2048`] file.
const JPEG_QUALITY: u8 = 85;

/// Never overwrite an existing export: photo.jpg -> photo (1).jpg, photo (2).jpg...
fn unique_destination(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "export".to_string());
    let extension = source.extension().map(|e| e.to_string_lossy().to_string());
    for index in 1..10_000 {
        let name = match extension.as_deref() {
            Some(ext) => format!("{stem} ({index}).{ext}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = directory.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem}-{}.export", std::process::id()))
}

/// Result of exporting media + metadata.
#[derive(Debug, Default, Clone)]
pub struct ExportReport {
    pub exported: usize,
    pub sidecars: usize,
    pub skipped: usize,
    pub failed: Vec<(PathBuf, String)>,
    /// Videos copied unchanged by a preset that converts photos.
    pub videos_as_originals: usize,
    /// Of those, the videos whose catalog rotation the copy leaves behind.
    pub videos_without_rotation: usize,
    /// The export stopped early because it was cancelled.
    pub cancelled: bool,
}

/// What an export writes for each photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPreset {
    /// The original files, plus an XMP sidecar for annotated items.
    Originals,
    /// A JPEG at most 2048 px on the long side with Lumina's edits applied,
    /// with no metadata. Videos are copied as originals.
    Jpeg2048,
}

impl LuminaEngine {
    /// Exports the given items into destination as `preset` says. Existing
    /// files are never overwritten; collisions get a numbered suffix. A
    /// missing or unreadable item is reported in `failed` and the rest still
    /// export. `cancel` is checked before each file, so a cancel finishes the
    /// file being written and stops there.
    pub fn export_items_with(
        &self,
        ids: &[MediaId],
        destination: &Path,
        preset: ExportPreset,
        cancel: &AtomicBool,
    ) -> Result<ExportReport> {
        fs::create_dir_all(destination)?;
        let jpeg = preset == ExportPreset::Jpeg2048;
        let item_edits = if jpeg {
            self.catalog
                .get_edits(ids)
                .context("could not read edits")?
        } else {
            HashMap::new()
        };
        let captions = self
            .catalog
            .get_captions_map()
            .context("could not read captions")?;
        let ratings = self
            .catalog
            .get_ratings_map()
            .context("could not read ratings")?;
        let labels = self
            .catalog
            .get_color_labels_map()
            .context("could not read color labels")?;
        let rights = self
            .catalog
            .get_rights(ids)
            .context("could not read rights")?;
        let edited = self
            .catalog
            .edited_flags(ids)
            .context("could not read edited dates and places")?;
        let mut report = ExportReport::default();

        for id in ids {
            if cancel.load(Ordering::Relaxed) {
                report.cancelled = true;
                break;
            }
            let Some(item) = self.catalog.get_item_by_id(*id)? else {
                report.skipped += 1;
                continue;
            };
            if !item.path.exists() {
                report
                    .failed
                    .push((item.path.clone(), "source file is missing".to_string()));
                continue;
            }
            if jpeg && item.media_type != MediaType::Video {
                let edits = edits::effective_edits(
                    item.media_type,
                    item_edits.get(id).copied().unwrap_or_default(),
                );
                match write_jpeg(&item, &edits, destination) {
                    Ok(()) => report.exported += 1,
                    Err(err) => {
                        // The detail may name a decoder; the report stays plain.
                        log::warn!("JPEG export of {} failed: {err:#}", item.path.display());
                        report
                            .failed
                            .push((item.path.clone(), "could not convert to JPEG".to_string()));
                    }
                }
                continue;
            }
            let destination_path = unique_destination(destination, &item.file_name);
            if let Err(err) = copy_new(&item.path, &destination_path) {
                report
                    .failed
                    .push((item.path.clone(), format!("could not copy file: {err}")));
                continue;
            }
            report.exported += 1;
            if jpeg {
                // The JPEG preset writes clean files: no sidecar for videos either.
                report.videos_as_originals += 1;
                if item_edits.get(id).is_some_and(|e| e.rotation != 0) {
                    report.videos_without_rotation += 1;
                }
                continue;
            }

            match self.sidecar_metadata_for(*id, &captions, &ratings, &labels, &rights, &edited) {
                Ok(metadata) => match xmp::write_sidecar(&destination_path, &metadata) {
                    Ok(SidecarOutcome::Written(_)) => report.sidecars += 1,
                    Ok(SidecarOutcome::Removed | SidecarOutcome::Unchanged) => {}
                    // The media name is fresh, but a sidecar left in the
                    // export folder under that name can still be in the way.
                    Ok(SidecarOutcome::SkippedForeign) => report.failed.push((
                        item.path.clone(),
                        "an XMP sidecar from another app is already there and was left unchanged"
                            .to_string(),
                    )),
                    Err(err) => report.failed.push((
                        item.path.clone(),
                        format!("could not write XMP sidecar: {err}"),
                    )),
                },
                Err(err) => report
                    .failed
                    .push((item.path.clone(), format!("could not read metadata: {err}"))),
            }
        }
        Ok(report)
    }
}

/// Decodes `item` the way the viewer does (EXIF orientation, then Lumina's
/// crop, rotation and tone), and writes `{stem}.jpg` into `destination`,
/// at most [`JPEG_LONG_SIDE`] on the long side and never larger than the
/// original. The encoder writes JFIF only: no EXIF, XMP or ICC, so no GPS.
// ponytail: decode bypasses the thumbnail disk cache, so a large export
// decodes every photo again; reuse the cache if exports prove slow.
fn write_jpeg(item: &MediaItem, edits: &Edits, destination: &Path) -> Result<()> {
    // sips scales up to the size asked for, so the original's size must be
    // known to never ask beyond it. The scanner cannot read every HEIC/RAW.
    let original = if item.width > 0 && item.height > 0 {
        (item.width, item.height)
    } else {
        crate::decoder::native_pixel_size(&item.path).unwrap_or((0, 0))
    };
    let mut size = edits::edit_decode_size(JPEG_LONG_SIDE, edits.crop, original);
    let long = original.0.max(original.1);
    if long > 0 {
        size = size.min(long);
    }
    let frame = DecoderPipeline::decode_media(item.id, item.media_type, &item.path, size)
        .context("could not decode the photo")?;
    let frame = edits::apply_edits(frame, edits);
    let mut image = image::RgbImage::from_raw(frame.width, frame.height, rgb8(&frame))
        .context("the decoded frame does not match its size")?;
    // A crop decodes larger than the target, and edit_decode_size can guess
    // the EXIF-oriented crop wrong, so cap the result itself.
    if image.width().max(image.height()) > JPEG_LONG_SIDE {
        image = image::DynamicImage::ImageRgb8(image)
            .resize(
                JPEG_LONG_SIDE,
                JPEG_LONG_SIDE,
                image::imageops::FilterType::Triangle,
            )
            .into_rgb8();
    }
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .encode(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgb8,
        )
        .context("could not encode the JPEG")?;

    let stem = Path::new(&item.file_name)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".to_string());
    let path = unique_destination(destination, &format!("{stem}.jpg"));
    create_new_with(&path, |file| file.write_all(&bytes))
        .with_context(|| format!("could not write {}", path.display()))
}

/// Copies `source` to a new file at `destination` with its metadata; on a
/// failure the reserved file is removed.
fn copy_new(source: &Path, destination: &Path) -> std::io::Result<()> {
    // Reserve the name first, so a file that appeared after
    // unique_destination picked it is never truncated. Then fs::copy, which
    // on macOS uses fcopyfile(COPYFILE_ALL): mode, dates and xattrs (Finder
    // tags) travel with the bytes.
    drop(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?,
    );
    if let Err(err) = fs::copy(source, destination) {
        remove_partial(destination);
        return Err(err);
    }
    Ok(())
}

/// Creates `path` (never replacing a file that is already there, even one
/// that appeared after [`unique_destination`] picked the name), fills it
/// and syncs it. On a failure the partial file is removed.
fn create_new_with(
    path: &Path,
    fill: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    if let Err(err) = fill(&mut file).and_then(|()| file.sync_all()) {
        drop(file);
        remove_partial(path);
        return Err(err);
    }
    Ok(())
}

/// Removes a partial export after a failed write; a failure is only logged.
fn remove_partial(path: &Path) {
    if let Err(remove) = fs::remove_file(path) {
        log::warn!(
            "could not remove partial export {}: {remove}",
            path.display()
        );
    }
}

/// The frame's pixels as packed RGB8, dropping alpha. Decoded frames are
/// usually BGRA8, which a JPEG encoder would read as blue-for-red.
fn rgb8(frame: &DecodedFrame) -> Vec<u8> {
    let (r, b) = match frame.format {
        PixelFormat::Rgba8 => (0, 2),
        PixelFormat::Bgra8 => (2, 0),
    };
    frame
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|pixel| [pixel[r], pixel[1], pixel[b]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb8_keeps_rgba_channel_order() {
        let frame = DecodedFrame {
            id: MediaId(1),
            width: 2,
            height: 1,
            format: PixelFormat::Rgba8,
            data: vec![255, 1, 2, 9, 3, 4, 250, 9],
        };
        assert_eq!(
            rgb8(&frame),
            vec![255, 1, 2, 3, 4, 250],
            "an RGBA frame is not swizzled; alpha is dropped"
        );
    }
}
