//! Settings › Downloads: how videos from links are downloaded.

use gpui::*;

use super::rows::{controls, row, section, value};

pub(super) fn render_downloads() -> AnyElement {
    let running = crate::app::downloads::MAX_RUNNING.to_string();
    let marker = running.clone();
    // ponytail: read-only until the concurrency setting lands (Design 15 Task 3).
    section(
        "TRANSFERS",
        Some("Applies to videos imported from a link".into()),
        [row(
            "Simultaneous downloads",
            None,
            controls().child(
                value(&running, true)
                    .debug_selector(move || format!("settings-concurrency-text:{marker}")),
            ),
        )
        .debug_selector(|| "settings-row-concurrency".to_string())],
    )
    .into_any_element()
}
