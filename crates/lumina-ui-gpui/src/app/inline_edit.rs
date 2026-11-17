//! The inspector's inline title, caption, date and location edits, single
//! and batch, and the date-shift and coordinate parsers they share.

use gpui::*;
use lumina_core::text::{clean_caption, clean_text, MAX_TEXT_CHARS};

use crate::app::LuminaApp;

impl LuminaApp {
    pub fn start_caption_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.caption);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Caption),
                initial_buffer: existing.clone(),
            };
            self.caption.input_active = true;
            self.caption.input_buffer = existing;
            cx.notify();
            return;
        }
        let existing = self
            .selected_item
            .as_ref()
            .and_then(|it| self.item_captions.get(&it.id))
            .cloned()
            .unwrap_or_default();
        self.caption.input_active = true;
        self.caption.input_buffer = existing;
        cx.notify();
    }

    pub fn commit_caption_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_caption_input(cx);
            return;
        }
        let caption = std::mem::take(&mut self.caption.input_buffer);
        self.caption.input_active = false;
        if let Some(item) = self.selected_item.clone() {
            let trimmed = clean_caption(&caption, MAX_TEXT_CHARS);
            // Cleaned like the stored value, so undo shows what it restores.
            let before = self
                .item_captions
                .get(&item.id)
                .map(|caption| clean_caption(caption, MAX_TEXT_CHARS))
                .filter(|caption| !caption.is_empty());
            let after = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.clone())
            };
            if before == after {
                cx.notify();
                return;
            }
            if let Some(ref value) = after {
                self.item_captions.insert(item.id, value.clone());
            } else {
                self.item_captions.remove(&item.id);
            }
            let result = if let Some(ref state) = self.grid_state {
                state.engine.set_caption(item.id, &trimmed)
            } else {
                Ok(())
            };
            if let Err(err) = result {
                if let Some(previous) = before {
                    self.item_captions.insert(item.id, previous);
                } else {
                    self.item_captions.remove(&item.id);
                }
                self.set_error(format!("Could not save caption: {err:#}"), cx);
                return;
            }
            self.push_undo(crate::app::undo::UndoAction::Caption {
                id: item.id,
                before,
                after,
            });
            self.sync_metadata_sidecars(&[item.id], cx);
            self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
            let message = if trimmed.is_empty() {
                "Caption removed"
            } else {
                "Caption saved"
            };
            self.set_last_action(message);
            self.show_undo_toast(message, cx);
        }
        cx.notify();
    }

    pub fn cancel_caption_input(&mut self, cx: &mut Context<Self>) {
        self.caption.input_active = false;
        self.caption.input_buffer.clear();
        let was_batch = self.caption.batch.field.take().is_some();
        self.caption.batch.initial_buffer.clear();
        if was_batch {
            // Restore the "Mixed" / shared value the batch row shows when idle.
            self.update_batch_summary();
        }
        cx.notify();
    }

    /// Draft a batch field opens with: the shared value, or empty when the
    /// selection disagrees so an unchanged Enter is a no-op.
    fn batch_draft(&self, field: &crate::app::state::BatchFieldSummary) -> String {
        if field.mixed {
            String::new()
        } else {
            field.common.clone().unwrap_or_default()
        }
    }

    fn commit_batch_caption_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.input_active = false;
        if field != Some(crate::app::state::BatchEditField::Caption) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never overwrite N items silently.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = clean_caption(&buffer, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self.apply_batch_caption(after, cx);
    }

    fn commit_batch_title_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.title_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.title_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Title) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = clean_text(&buffer, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        self.apply_batch_title(after, cx);
    }

    pub fn start_title_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.title);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Title),
                initial_buffer: existing.clone(),
            };
            self.caption.title_input_active = true;
            self.caption.title_input_buffer = existing;
            cx.notify();
            return;
        }
        let existing = self
            .selected_item
            .as_ref()
            .and_then(|item| item.title.clone())
            .unwrap_or_default();
        self.caption.title_input_active = true;
        self.caption.title_input_buffer = existing;
        cx.notify();
    }

    pub fn commit_title_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_title_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.title_input_buffer);
        self.caption.title_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let trimmed = clean_text(&value, MAX_TEXT_CHARS);
        let after = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
        // Cleaned like the stored value, so undo shows what it restores.
        let before = item
            .title
            .as_deref()
            .map(|title| clean_text(title, MAX_TEXT_CHARS))
            .filter(|title| !title.is_empty());
        if before == after {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_title(item.id, after.as_deref())
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save title: {err:#}"), cx);
            return;
        }
        if let Some(ref mut current) = self.selected_item {
            current.title = after.clone();
        }
        let removed = after.is_none();
        self.push_undo(crate::app::undo::UndoAction::Title {
            id: item.id,
            before,
            after,
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        let message = if removed {
            "Title removed"
        } else {
            "Title saved"
        };
        self.set_last_action(message);
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    pub fn cancel_title_input(&mut self, cx: &mut Context<Self>) {
        self.caption.title_input_active = false;
        self.caption.title_input_buffer.clear();
        let was_batch = self.caption.batch.field.take().is_some();
        self.caption.batch.initial_buffer.clear();
        if was_batch {
            self.update_batch_summary();
        }
        cx.notify();
    }

    pub fn start_date_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.date);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Date),
                initial_buffer: existing.clone(),
            };
            self.caption.date_input_active = true;
            self.caption.date_input_buffer = existing;
            cx.notify();
            return;
        }
        let current = self
            .selected_item
            .as_ref()
            .map(|item| item.timestamp_epoch_ms)
            .unwrap_or(0);
        let buffer = chrono::DateTime::from_timestamp_millis(current)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        self.caption.date_input_active = true;
        self.caption.date_input_buffer = buffer;
        cx.notify();
    }

    pub fn commit_date_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_date_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.date_input_buffer);
        self.caption.date_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let Some(epoch_ms) = parse_edited_timestamp(&value, item.timestamp_epoch_ms) else {
            self.set_error(DATE_INPUT_ERROR, cx);
            return;
        };
        if epoch_ms == item.timestamp_epoch_ms {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_timestamp(item.id, epoch_ms)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save date: {err:#}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Date {
            id: item.id,
            before: item.timestamp_epoch_ms,
            after: epoch_ms,
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        if let Some(updated) = self
            .grid_state
            .as_ref()
            .and_then(|state| state.engine.get_item_by_id(item.id))
        {
            self.set_selected_from_catalog(updated);
        }
        self.refresh_album_counts();
        self.set_last_action("Date updated");
        self.show_undo_toast("Date updated", cx);
        cx.notify();
    }

    pub fn cancel_date_input(&mut self, cx: &mut Context<Self>) {
        self.caption.date_input_active = false;
        self.caption.date_input_buffer.clear();
        if self.caption.batch.field == Some(crate::app::state::BatchEditField::Date) {
            self.caption.batch.field = None;
            self.caption.batch.initial_buffer.clear();
            self.update_batch_summary();
        }
        cx.notify();
    }

    pub fn start_location_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            let existing = self.batch_draft(&self.batch_metadata.location);
            self.caption.batch = crate::app::state::BatchEditState {
                field: Some(crate::app::state::BatchEditField::Location),
                initial_buffer: existing.clone(),
            };
            self.caption.location_input_active = true;
            self.caption.location_input_buffer = existing;
            cx.notify();
            return;
        }
        let buffer = self
            .selected_item
            .as_ref()
            .and_then(|item| match (item.latitude, item.longitude) {
                (Some(lat), Some(lon)) => Some(format!("{lat:.6}, {lon:.6}")),
                _ => None,
            })
            .unwrap_or_default();
        self.caption.location_input_active = true;
        self.caption.location_input_buffer = buffer;
        cx.notify();
    }

    pub fn commit_location_input(&mut self, cx: &mut Context<Self>) {
        if self.selected_indices.len() > 1 {
            self.commit_batch_location_input(cx);
            return;
        }
        let value = std::mem::take(&mut self.caption.location_input_buffer);
        self.caption.location_input_active = false;
        let Some(item) = self.selected_item.clone() else {
            cx.notify();
            return;
        };
        let (lat, lon) = match parse_location_coordinates(&value) {
            Ok(coords) => coords,
            Err(message) => {
                self.set_error(message, cx);
                return;
            }
        };
        if (lat, lon) == (item.latitude, item.longitude) {
            cx.notify();
            return;
        }
        let result = if let Some(ref state) = self.grid_state {
            state.engine.set_location(item.id, lat, lon)
        } else {
            Ok(())
        };
        if let Err(err) = result {
            self.set_error(format!("Could not save location: {err:#}"), cx);
            return;
        }
        self.push_undo(crate::app::undo::UndoAction::Location {
            id: item.id,
            before: (item.latitude, item.longitude),
            after: (lat, lon),
        });
        self.sync_metadata_sidecars(&[item.id], cx);
        self.refresh_counts_if_rules_use(lumina_core::RuleField::Query);
        if let Some(updated) = self
            .grid_state
            .as_ref()
            .and_then(|state| state.engine.get_item_by_id(item.id))
        {
            self.set_selected_from_catalog(updated);
        }
        let message = if lat.is_some() {
            "Location updated"
        } else {
            "Location removed"
        };
        self.set_last_action(message);
        self.show_undo_toast(message, cx);
        cx.notify();
    }

    /// Get Info "Remove location": the same path as submitting an empty
    /// location, so it is saved, undoable and reports errors.
    pub fn remove_location_selected(&mut self, cx: &mut Context<Self>) {
        self.caption.location_input_buffer.clear();
        self.caption.location_input_active = true;
        self.commit_location_input(cx);
    }

    pub fn cancel_location_input(&mut self, cx: &mut Context<Self>) {
        self.caption.location_input_active = false;
        self.caption.location_input_buffer.clear();
        if self.caption.batch.field == Some(crate::app::state::BatchEditField::Location) {
            self.caption.batch.field = None;
            self.caption.batch.initial_buffer.clear();
            self.update_batch_summary();
        }
        cx.notify();
    }

    fn commit_batch_date_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.date_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.date_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Date) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never rewrite the whole selection.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = buffer.trim().to_string();
        if trimmed.is_empty() {
            self.set_error(DATE_INPUT_ERROR, cx);
            self.update_batch_summary();
            cx.notify();
            return;
        }
        self.apply_batch_date(&trimmed, cx);
    }

    fn commit_batch_location_input(&mut self, cx: &mut Context<Self>) {
        let buffer = std::mem::take(&mut self.caption.location_input_buffer);
        let initial = std::mem::take(&mut self.caption.batch.initial_buffer);
        let field = self.caption.batch.field.take();
        self.caption.location_input_active = false;
        if field != Some(crate::app::state::BatchEditField::Location) {
            self.update_batch_summary();
            cx.notify();
            return;
        }
        if buffer == initial {
            // Untouched draft: never rewrite the whole selection.
            self.update_batch_summary();
            cx.notify();
            return;
        }
        let trimmed = buffer.trim().to_string();
        self.apply_batch_location(&trimmed, cx);
    }
}

/// Parses the inspector's location draft: decimal degrees as
/// `latitude, longitude`, accepting comma or whitespace separators. An empty
/// draft clears the location. Shared by the single-item and batch editors so
/// both accept exactly the same syntax.
pub(crate) fn parse_location_coordinates(
    input: &str,
) -> Result<(Option<f64>, Option<f64>), &'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok((None, None));
    }
    let parts: Vec<&str> = trimmed
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 2 {
        return Err("Enter coordinates as latitude, longitude");
    }
    let (Ok(lat), Ok(lon)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) else {
        return Err("Coordinates must be decimal numbers");
    };
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err("Latitude must be -90..90 and longitude -180..180");
    }
    Ok((Some(lat), Some(lon)))
}

/// Shown when the inspector's date draft is neither a date nor a shift.
pub(crate) const DATE_INPUT_ERROR: &str = "Enter a date as YYYY-MM-DD, or a shift like +2h or -1d";

/// Parses a signed date shift into milliseconds: `+2h`, `-1d`, `+1d 3h 15m`
/// (units d/h/m/s) or `-0:30` (`H:MM`). The sign is required, so a bare
/// `2h` is not a shift.
pub(crate) fn parse_date_shift(input: &str) -> Option<i64> {
    fn number(digits: &str) -> Option<i64> {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }
    let trimmed = input.trim();
    let sign = match trimmed.chars().next()? {
        '+' => 1,
        '-' => -1,
        _ => return None,
    };
    let body = trimmed[1..].trim();
    let seconds = if let Some((hours, minutes)) = body.split_once(':') {
        let minutes = number(minutes).filter(|&m| m < 60 && minutes.len() == 2)?;
        number(hours)?
            .checked_mul(3600)?
            .checked_add(minutes * 60)?
    } else {
        let mut total: i64 = 0;
        let mut rest = body;
        if rest.is_empty() {
            return None;
        }
        while !rest.is_empty() {
            let end = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            let value = number(&rest[..end])?;
            let mut tail = rest[end..].chars();
            let unit = match tail.next()? {
                'd' => 86_400,
                'h' => 3_600,
                'm' => 60,
                's' => 1,
                _ => return None,
            };
            total = total.checked_add(value.checked_mul(unit)?)?;
            rest = tail.as_str().trim_start();
        }
        total
    };
    seconds.checked_mul(1000).map(|ms| sign * ms)
}

/// Parses the inspector's date draft against an item's current timestamp: a
/// shift (`+2h`, see [`parse_date_shift`]) moves it, `YYYY-MM-DD HH:MM:SS`
/// replaces it, and `YYYY-MM-DD` changes the day and keeps the time of day.
pub(crate) fn parse_edited_timestamp(input: &str, current_epoch_ms: i64) -> Option<i64> {
    let trimmed = input.trim();
    if let Some(delta) = parse_date_shift(trimmed) {
        // Out of chrono's range is refused here, not by the catalog write.
        return current_epoch_ms
            .checked_add(delta)
            .filter(|&ms| chrono::DateTime::from_timestamp_millis(ms).is_some());
    }
    if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M:%S") {
        return Some(datetime.and_utc().timestamp_millis());
    }
    if let Ok(date) = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let time = chrono::DateTime::from_timestamp_millis(current_epoch_ms)
            .map(|dt| dt.time())
            .unwrap_or_default();
        return Some(date.and_time(time).and_utc().timestamp_millis());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{parse_date_shift, parse_edited_timestamp};

    const MIN: i64 = 60_000;

    #[core::prelude::v1::test]
    fn parse_date_shift_units_and_signs() {
        assert_eq!(parse_date_shift("+2h"), Some(120 * MIN), "hours");
        assert_eq!(
            parse_date_shift("-1d"),
            Some(-1440 * MIN),
            "a minus shifts back"
        );
        assert_eq!(
            parse_date_shift("+1d 3h 15m"),
            Some((1440 + 180 + 15) * MIN),
            "units add up"
        );
        assert_eq!(parse_date_shift(" +45s "), Some(45_000), "seconds, trimmed");
        assert_eq!(parse_date_shift("-0:30"), Some(-30 * MIN), "H:MM form");
        assert_eq!(parse_date_shift("+1:05"), Some(65 * MIN), "H:MM form");
    }

    #[core::prelude::v1::test]
    fn parse_date_shift_rejects_garbage() {
        for input in [
            "+",
            "2h",
            "+2x",
            "",
            "-",
            "+h",
            "+2",
            "+2h x",
            "+1:5",
            "+1:60",
            "+:30",
            "+-2h",
            "2024-05-01",
        ] {
            assert_eq!(parse_date_shift(input), None, "{input:?} is not a shift");
        }
    }

    #[core::prelude::v1::test]
    fn date_shift_out_of_range_is_refused() {
        assert_eq!(
            parse_edited_timestamp("+999999999d", 0),
            None,
            "a shift past chrono's range is an input error, not a failed write"
        );
        assert_eq!(parse_edited_timestamp("+1d", 0), Some(86_400_000));
    }
}
