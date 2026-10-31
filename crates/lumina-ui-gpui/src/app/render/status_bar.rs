use gpui::*;

use crate::app::LuminaApp;
use crate::theme;

/// Right-hand status text: scan progress takes priority, then the last action,
/// then a static hint.
pub(crate) fn format_status_right(
    is_scanning: bool,
    scan_progress: usize,
    last_action: Option<&str>,
) -> String {
    if is_scanning {
        return format!("Scanning… {} items found", scan_progress);
    }
    if let Some(action) = last_action {
        return action.to_string();
    }
    "Right-click any item for actions".to_string()
}

impl LuminaApp {
    pub(crate) fn status_right_text(&self) -> String {
        let (is_scanning, scan_progress) = self
            .grid_state
            .as_ref()
            .map(|state| (state.is_scanning, state.scan_progress))
            .unwrap_or((false, 0));
        format_status_right(is_scanning, scan_progress, self.last_action.as_deref())
    }

    pub(crate) fn render_status_bar(&self, total_count: usize) -> impl IntoElement {
        let (is_scanning, _) = self
            .grid_state
            .as_ref()
            .map(|state| (state.is_scanning, state.scan_progress))
            .unwrap_or((false, 0));

        div()
            .debug_selector(|| "status-bar".to_string())
            .h(px(28.0))
            .w_full()
            .bg(theme::bg_app())
            .border_t_1()
            .border_color(theme::border())
            .px_4()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(div().text_xs().text_color(theme::text_subtle()).child({
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
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(if is_scanning {
                        theme::text()
                    } else {
                        theme::text_subtle()
                    })
                    .child(self.status_right_text()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn scan_progress_takes_priority() {
        assert_eq!(
            format_status_right(true, 42, Some("Tag renamed")),
            "Scanning… 42 items found"
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
