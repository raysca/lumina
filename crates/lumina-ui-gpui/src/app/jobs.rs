//! Export and XMP import, run off the UI thread (AGENTS §1). Each is one
//! background task with a status-bar line and a final toast or error banner.

use std::path::{Path, PathBuf};

use gpui::*;
use lumina_core::{ExportReport, ImportReport};

use crate::app::LuminaApp;
use crate::i18n::t;

/// The export and XMP import running now, as their item counts. App-level,
/// so a job started in one library still blocks a second one after a switch.
#[derive(Debug, Default)]
pub(crate) struct ForegroundJobs {
    pub(crate) export: Option<usize>,
    pub(crate) xmp: Option<usize>,
}

impl ForegroundJobs {
    /// The status-bar line for a running job, export first.
    pub(crate) fn status_text(&self) -> Option<String> {
        let line = |key: &str, n: usize| {
            let key = format!("{key}_{}", if n == 1 { "one" } else { "other" });
            t(&key).replace("{n}", &n.to_string())
        };
        self.export
            .map(|n| line("status.exporting", n))
            .or_else(|| self.xmp.map(|n| line("status.reading_xmp", n)))
    }
}

impl LuminaApp {
    /// Copies the selection into `folder` in the background, writing XMP
    /// sidecars for annotated items. A second export while one runs is refused.
    pub fn export_to_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        if self.jobs.export.is_some() {
            self.show_toast(t("jobs.export_running"), cx);
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
        let destination = folder.clone();
        let task = cx
            .background_executor()
            .spawn(async move { engine.export_items(&ids, &destination) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            // The app may be gone; then there is no job left to clear.
            let _ = this.update(cx, |app, cx| app.finish_export(&folder, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn finish_export(
        &mut self,
        folder: &Path,
        result: anyhow::Result<ExportReport>,
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
                self.set_last_action(summary);
                if report.failed.is_empty() {
                    self.show_toast(format!("Exported {} item(s)", report.exported), cx);
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
            self.show_toast(t("jobs.xmp_running"), cx);
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
