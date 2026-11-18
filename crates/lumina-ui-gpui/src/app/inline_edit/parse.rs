//! The date-shift and coordinate parsers the inline and batch edits share.

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
