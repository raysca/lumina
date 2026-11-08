//! Exporting catalogued media to a folder the user picked.

use crate::{xmp, LuminaEngine, MediaId, SidecarOutcome};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

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
}

impl LuminaEngine {
    /// Copies the given items into destination, writing an XMP sidecar for each
    /// item that has rating/tags/caption metadata. Existing files are never
    /// overwritten; collisions get a numbered suffix.
    pub fn export_items(&self, ids: &[MediaId], destination: &Path) -> Result<ExportReport> {
        fs::create_dir_all(destination)?;
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
            let destination_path = unique_destination(destination, &item.file_name);
            if let Err(err) = fs::copy(&item.path, &destination_path) {
                report
                    .failed
                    .push((item.path.clone(), format!("could not copy file: {err}")));
                continue;
            }
            report.exported += 1;

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
