use gpui::*;

use crate::app::LuminaApp;
use crate::theme;

/// Right-hand status text: opening/import/scan progress takes priority,
/// then the last action, then a static hint.
#[cfg(test)]
pub(crate) fn format_status_right(
    is_scanning: bool,
    scan_progress: usize,
    last_action: Option<&str>,
) -> String {
    format_status_right_with_totals(is_scanning, scan_progress, 0, 0, last_action)
}

pub(crate) fn format_status_right_with_totals(
    is_scanning: bool,
    scan_progress: usize,
    scan_total: usize,
    scan_counting: usize,
    last_action: Option<&str>,
) -> String {
    if is_scanning {
        if scan_total > 0 {
            return format!(
                "Scanning… {} of {}",
                crate::i18n::count(scan_progress),
                crate::i18n::count(scan_total)
            );
        }
        if scan_counting > 0 {
            return format!(
                "Counting files… {} found",
                crate::i18n::count(scan_counting)
            );
        }
        if scan_progress > 0 {
            return format!(
                "Scanning… {} items checked",
                crate::i18n::count(scan_progress)
            );
        }
        return "Scanning…".to_string();
    }
    if let Some(action) = last_action {
        return action.to_string();
    }
    "Right-click any item for actions".to_string()
}

impl LuminaApp {
    pub(crate) fn status_right_text(&self) -> String {
        if let Some(path) = self.welcome.opening_path() {
            return format!("Opening {}…", crate::app::recents::library_name(path));
        }
        if self.import.is_stopping() {
            return "Stopping import…".to_string();
        }
        if let Some((done, total)) = self.import.progress {
            return format!("Importing… {done} of {total}");
        }
        if let Some(job) = self.jobs.status_text() {
            return job;
        }
        let (is_scanning, scan_progress, scan_total, scan_counting) = self
            .grid_state
            .as_ref()
            .map(|state| {
                (
                    state.is_scanning,
                    state.scan_progress,
                    state.scan_total,
                    state.scan_counting,
                )
            })
            .unwrap_or((false, 0, 0, 0));
        format_status_right_with_totals(
            is_scanning,
            scan_progress,
            scan_total,
            scan_counting,
            self.last_action.as_deref(),
        )
    }

    pub(crate) fn render_status_bar(
        &self,
        total_count: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let importing = self.import.progress;
        let opening = self.welcome.opening();
        let (is_scanning, scan_progress, scan_total) = self
            .grid_state
            .as_ref()
            .map(|state| (state.is_scanning, state.scan_progress, state.scan_total))
            .unwrap_or((false, 0, 0));

        div()
            .debug_selector(|| "status-bar".to_string())
            .h(px(28.0))
            .w_full()
            .bg(theme::bg_0())
            .border_t_1()
            .border_color(theme::line())
            .px_4()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_size(theme::TEXT_SMALL)
                    .text_color(theme::t2())
                    .child({
                        let sel_count = if !self.selected_indices.is_empty() {
                            self.selected_indices.len()
                        } else if self.selected_item.is_some() {
                            1
                        } else {
                            0
                        };
                        let year_str = if let Some(y) = self.active_year {
                            format!(" · {}", y)
                        } else {
                            String::new()
                        };
                        format!("{} of {} selected{}", sel_count, total_count, year_str)
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(theme::TEXT_SMALL)
                            .text_color(if opening || is_scanning || importing.is_some() {
                                theme::t1()
                            } else {
                                theme::t2()
                            })
                            .child(self.status_right_text()),
                    )
                    .children(importing.map(|(done, total)| {
                        crate::views::import_dialog::progress_bar(
                            "import-progress",
                            done as f32 / total.max(1) as f32,
                            96.0,
                        )
                    }))
                    .children(
                        (!opening && importing.is_none() && is_scanning && scan_total > 0).then(
                            || {
                                crate::views::import_dialog::progress_bar(
                                    "scan-progress",
                                    scan_progress as f32 / scan_total.max(1) as f32,
                                    96.0,
                                )
                            },
                        ),
                    )
                    .children((!opening && importing.is_none() && is_scanning).then(|| {
                        div()
                            .id("scan-stop-running")
                            .debug_selector(|| "scan-stop-running".to_string())
                            .role(Role::Button)
                            .aria_label("Stop scanning")
                            .px_2()
                            .rounded(theme::RADIUS_SM)
                            .cursor_pointer()
                            .text_size(theme::TEXT_SMALL)
                            .text_color(theme::t1())
                            .hover(|s| s.bg(theme::bg_3()))
                            .tooltip(theme::tooltip_label(
                                "Stop scanning (⌘.); Rescan (⌘R) later",
                            ))
                            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                this.stop_first_scan(cx);
                            }))
                            .child("Stop")
                    }))
                    .children(
                        (importing.is_some() && !self.import.is_stopping()).then(|| {
                            div()
                                .id("import-cancel-running")
                                .debug_selector(|| "import-cancel-running".to_string())
                                .px_2()
                                .rounded(theme::RADIUS_SM)
                                .cursor_pointer()
                                .text_size(theme::TEXT_SMALL)
                                .text_color(theme::t1())
                                .hover(|s| s.bg(theme::bg_3()))
                                .tooltip(theme::tooltip_label("Stop after the current file"))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.cancel_running_import(cx);
                                }))
                                .child("Cancel")
                        }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn scan_progress_takes_priority() {
        assert_eq!(
            format_status_right_with_totals(true, 42, 100, 0, Some("Tag renamed")),
            "Scanning… 42 of 100"
        );
        assert_eq!(
            format_status_right_with_totals(true, 0, 0, 500, Some("Tag renamed")),
            "Counting files… 500 found"
        );
        assert_eq!(
            format_status_right(true, 42, Some("Tag renamed")),
            "Scanning… 42 items checked"
        );
    }

    #[core::prelude::v1::test]
    fn last_action_beats_static_hint() {
        assert_eq!(
            format_status_right(false, 0, Some("Moved 3 items to Trash")),
            "Moved 3 items to Trash"
        );
    }

    #[core::prelude::v1::test]
    fn falls_back_to_shortcut_hint() {
        assert_eq!(
            format_status_right(false, 0, None),
            "Right-click any item for actions"
        );
    }
}
