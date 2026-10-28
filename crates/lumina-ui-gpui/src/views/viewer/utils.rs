use crate::theme;
use gpui::*;

pub fn render_shortcut_hint(keys: Vec<&str>, label: &str) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .children(keys.into_iter().map(|k| {
            div()
                .px_1p5()
                .py_0p5()
                .bg(theme::bg_control())
                .border_1()
                .border_color(theme::border())
                .rounded_sm()
                .font_family(theme::FONT_MONO)
                .text_size(px(10.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text())
                .child(k.to_string())
        }))
        .child(
            div()
                .text_color(theme::text_muted())
                .child(label.to_string()),
        )
}

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
