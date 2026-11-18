//! Export and XMP import, run off the UI thread (AGENTS §1). Each is one
//! background task with a status-bar line and a final toast or error banner.

use std::path::{Path, PathBuf};

use gpui::*;
use lumina_core::{ExportPreset, ExportReport, ImportReport};

use crate::app::LuminaApp;
use crate::i18n::t;

/// The export and XMP import running now, as their item counts, and
/// whether a Duplicates Merge all is running. App-level, so a job started
/// in one library still blocks a second one after a switch.
#[derive(Debug, Default)]
pub(crate) struct ForegroundJobs {
    pub(crate) export: Option<usize>,
    pub(crate) xmp: Option<usize>,
    pub(crate) merge_all: bool,
}

impl ForegroundJobs {
    /// The status-bar line for a running job, export first.
    pub(crate) fn status_text(&self) -> Option<String> {
        let line = |key: &str, n: usize| {
            t(&crate::i18n::plural_key(key, n)).replace("{n}", &n.to_string())
        };
        self.export
            .map(|n| line("status.exporting", n))
            .or_else(|| self.xmp.map(|n| line("status.reading_xmp", n)))
    }
}

impl LuminaApp {
    /// Exports the selection into `folder` as `preset` in the background:
    /// originals with XMP sidecars, or edited JPEGs. A second export while
    /// one runs is refused.
    pub fn export_to_folder(
        &mut self,
        folder: PathBuf,
        preset: ExportPreset,
        cx: &mut Context<Self>,
    ) {
        if self.jobs.export.is_some() {
            self.show_warning_toast(t("jobs.export_running"), cx);
            return;
        }
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before exporting", cx);
            return;
        };
        self.jobs.export = Some(ids.len());
        // Originals leave Lumina's edits behind; the report says so for
        // edited items. The JPEG preset applies them.
        let edited = preset == ExportPreset::Originals
            && ids.iter().any(|id| self.edits.edited_ids.contains(id));
        let destination = folder.clone();
        // ponytail: no cancel control yet; the engine checks this between files.
        let task = cx.background_executor().spawn(async move {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            // A panic in a decoder must end this job with an error, not take
            // down the app or leave `jobs.export` set forever.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.export_items_with(&ids, &destination, preset, &cancel)
            }))
            .unwrap_or_else(|_| Err(anyhow::anyhow!("the export stopped unexpectedly")))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // The app may be gone; then there is no job left to clear.
            let _ = this.update(cx, |app, cx| app.finish_export(&folder, result, edited, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_export(
        &mut self,
        folder: &Path,
        result: anyhow::Result<ExportReport>,
        edited: bool,
        cx: &mut Context<Self>,
    ) {
        self.jobs.export = None;
        cx.notify();
        match result {
            Ok(report) => {
                let mut summary = format!(
                    "Exported {} item(s) to {}",
                    report.exported,
                    folder.display()
                );
                if report.sidecars > 0 {
                    summary.push_str(&format!(" ({} XMP sidecar(s))", report.sidecars));
                }
                let mut note = if edited {
                    "; the originals were exported without Lumina's edits or rotation".to_string()
                } else {
                    String::new()
                };
                if report.videos_as_originals > 0 {
                    let key = if report.videos_as_originals == 1 {
                        "export.videos_as_originals_one"
                    } else {
                        "export.videos_as_originals_other"
                    };
                    note.push_str("; ");
                    note.push_str(&t(key).replace("{n}", &report.videos_as_originals.to_string()));
                    if report.videos_without_rotation > 0 {
                        note.push(' ');
                        note.push_str(
                            &t("export.videos_rotation_dropped")
                                .replace("{n}", &report.videos_without_rotation.to_string()),
                        );
                    }
                }
                summary.push_str(&note);
                self.set_last_action(summary);
                if report.failed.is_empty() {
                    self.show_toast(format!("Exported {} item(s){note}", report.exported), cx);
                } else {
                    let details = report
                        .failed
                        .iter()
                        .take(3)
                        .map(|(path, error)| format!("{}: {error}", path.display()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.set_error(
                        format!("{} export(s) failed: {details}", report.failed.len()),
                        cx,
                    );
                }
            }
            Err(err) => self.set_error(format!("Export failed: {err:#}"), cx),
        }
    }

    /// Imports rating/title/caption/tags from adjacent XMP sidecars for the
    /// selection in the background. A second import while one runs is refused.
    pub fn import_sidecars_selected(&mut self, cx: &mut Context<Self>) {
        if self.jobs.xmp.is_some() {
            self.show_warning_toast(t("jobs.xmp_running"), cx);
            return;
        }
        let ids = self.get_selected_ids();
        if ids.is_empty() {
            return;
        }
        let Some(engine) = self.grid_state.as_ref().map(|state| state.engine.clone()) else {
            self.set_error("Open a library before importing metadata", cx);
            return;
        };
        self.jobs.xmp = Some(ids.len());
        let generation = self.library_generation;
        let task = cx
            .background_executor()
            .spawn(async move { engine.import_sidecars(&ids) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // The app may be gone; then there is no job left to clear.
            let _ = this.update(cx, |app, cx| app.finish_xmp_import(generation, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_xmp_import(
        &mut self,
        generation: u64,
        result: anyhow::Result<ImportReport>,
        cx: &mut Context<Self>,
    ) {
        self.jobs.xmp = None;
        cx.notify();
        match result {
            Ok(report) => {
                // The import wrote the library it started in; a newer library's
                // maps and selection have nothing to refresh from it.
                if generation == self.library_generation {
                    self.refresh_after_history(cx);
                }
                let mut summary = format!(
                    "Imported XMP metadata: {} sidecar(s), {} updated",
                    report.imported, report.updated
                );
                if report.skipped > 0 {
                    summary.push_str(&format!(", {} without sidecar", report.skipped));
                }
                self.set_last_action(summary);
                if !report.failed.is_empty() {
                    let details = report
                        .failed
                        .iter()
                        .take(3)
                        .map(|(path, error)| format!("{}: {error}", path.display()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    self.set_error(
                        format!(
                            "{} sidecar import(s) failed: {details}",
                            report.failed.len()
                        ),
                        cx,
                    );
                } else {
                    self.show_toast(
                        format!(
                            "Imported {} sidecar(s), {} updated",
                            report.imported, report.updated
                        ),
                        cx,
                    );
                }
            }
            Err(err) => self.set_error(format!("XMP import failed: {err:#}"), cx),
        }
    }
}
