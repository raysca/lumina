pub fn format_playback_speed(rate: f32) -> String {
    if (rate - 0.25).abs() < 0.01 {
        "0.25 ×".to_string()
    } else if (rate - 0.5).abs() < 0.01 {
        "0.5 ×".to_string()
    } else if (rate - 1.0).abs() < 0.01 {
        "1 ×".to_string()
    } else if (rate - 1.5).abs() < 0.01 {
        "1.5 ×".to_string()
    } else if (rate - 2.0).abs() < 0.01 {
        "2 ×".to_string()
    } else if (rate - 4.0).abs() < 0.01 {
        "4 ×".to_string()
    } else {
        let trimmed = format!("{:.2}", rate);
        let clean = trimmed.trim_end_matches('0').trim_end_matches('.');
        format!("{clean} ×")
    }
}
