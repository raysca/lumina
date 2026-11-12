/// "1.0×", "1.5×", "0.25×": at least one decimal, at most two, as the design's
/// speed pill shows.
pub fn format_playback_speed(rate: f32) -> String {
    let two = format!("{rate:.2}");
    let label = two.strip_suffix('0').unwrap_or(&two);
    format!("{label}×")
}
