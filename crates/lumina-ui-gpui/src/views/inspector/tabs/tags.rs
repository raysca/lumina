use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use gpui::base::input::{Input, InputState};
use gpui::*;

/// Library tags offered under the input; more would crowd the editor.
const MAX_SUGGESTIONS: usize = 6;

pub fn render_tags_tab(
    weak_app: WeakEntity<LuminaApp>,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    field_input: Option<Entity<InputState>>,
) -> AnyElement {
    render_tags_section(
        weak_app,
        "ON THIS ITEM",
        tags,
        library_tag_counts,
        tag_input_active,
        tag_input_buffer,
        field_input,
    )
}

/// Up to six library tags not already present that start with the typed text
/// (case-insensitive), most used first.
pub(crate) fn tag_suggestions<'a>(
    tags: &[String],
    library_tag_counts: &'a [(String, usize)],
    typed: &str,
) -> Vec<&'a str> {
    let typed = typed.trim().to_lowercase();
    library_tag_counts
        .iter()
        .map(|(tag, _)| tag.as_str())
        .filter(|tag| !tags.iter().any(|t| t.to_lowercase() == tag.to_lowercase()))
        .filter(|tag| tag.to_lowercase().starts_with(&typed))
        .take(MAX_SUGGESTIONS)
        .collect()
}

/// The tag editor (v3): chips on the item, an always-visible ADD field where
/// Enter adds, dashed suggestion chips, and a link to the Tag Manager.
pub fn render_tags_section(
    weak_app: WeakEntity<LuminaApp>,
    heading: &'static str,
    tags: &[String],
    library_tag_counts: &[(String, usize)],
    tag_input_active: bool,
    tag_input_buffer: &str,
    field_input: Option<Entity<InputState>>,
) -> AnyElement {
    let typed = if tag_input_active {
        tag_input_buffer
    } else {
        ""
    };
    let suggestions = tag_suggestions(tags, library_tag_counts, typed);
    let app_start_tag = weak_app.clone();
    let app_manager = weak_app.clone();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .child(theme::section_label(heading))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(format!("{} tags", tags.len())),
                ),
        )
        .child(if tags.is_empty() {
            div()
                .text_xs()
                .text_color(theme::text_subtle())
                .child("No tags yet.")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(tags.iter().map(|tag| {
                    let tag_name = tag.clone();
                    let filter_query = if tag_name.contains(' ') {
                        format!("tag:\"{}\"", tag_name)
                    } else {
                        format!("tag:{}", tag_name)
                    };
                    let app_filter = weak_app.clone();
                    let app_remove = weak_app.clone();
                    let tag_for_remove = tag_name.clone();

                    div()
                        .px_2()
                        .py_0p5()
                        .rounded(theme::RADIUS_MD)
                        .bg(theme::selection_bg())
                        .border_1()
                        .border_color(theme::selection_border())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1p5()
                        .child(
                            div()
                                .id(ElementId::Name(format!("filter-tag-{}", tag_name).into()))
                                .debug_selector({
                                    let id = format!("filter-tag-{}", tag_name);
                                    move || id.clone()
                                })
                                .tooltip(theme::tooltip_label("Show items with this tag"))
                                .cursor_pointer()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme::selection_fg())
                                .child(tag_name.clone())
                                .on_click(move |_e, _window, cx| {
                                    if let Some(app) = app_filter.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.set_search_query(&filter_query, cx);
                                        });
                                    }
                                }),
                        )
                        .child(
                            div()
                                .id(ElementId::Name(format!("remove-tag-{}", tag_name).into()))
                                .tooltip(theme::tooltip_label("Remove tag"))
                                .cursor_pointer()
                                .child(render_icon(Icon::X, px(10.0), theme::selection_fg()))
                                .on_click(move |_e, _window, cx| {
                                    if let Some(app) = app_remove.upgrade() {
                                        app.update(cx, |this, cx| {
                                            this.batch_remove_tag(tag_for_remove.clone(), cx);
                                        });
                                    }
                                }),
                        )
                }))
                .into_any_element()
        })
        .child(theme::section_label("ADD"))
        // Always visible; clicking focuses it, Enter adds, Esc clears.
        .child(
            div()
                .id("tag-input")
                .debug_selector(|| "tag-input".to_string())
                .w_full()
                .h(px(28.0))
                .px_2p5()
                .flex()
                .items_center()
                .rounded(theme::RADIUS_MD)
                .bg(theme::bg_app())
                .border_1()
                .border_color(if tag_input_active {
                    theme::focus_ring()
                } else {
                    theme::border()
                })
                .cursor_text()
                .text_size(theme::TEXT_BODY)
                .child(match field_input.filter(|_| tag_input_active) {
                    Some(input) => div()
                        .flex_1()
                        .text_color(theme::text())
                        .child(Input::new(&input)),
                    None => div().text_color(theme::text_subtle()).child("Add a tag…"),
                })
                .on_click(move |_e, _window, cx| {
                    if !tag_input_active {
                        if let Some(app) = app_start_tag.upgrade() {
                            app.update(cx, |this, cx| this.start_tag_input(cx));
                        }
                    }
                }),
        )
        .children((!suggestions.is_empty()).then(|| {
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_1()
                .children(suggestions.into_iter().map(|suggested| {
                    let tag_str = suggested.to_string();
                    let app_suggest = weak_app.clone();
                    div()
                        .id(ElementId::Name(format!("suggest-tag-{}", suggested).into()))
                        .debug_selector({
                            let id = format!("suggest-tag-{}", suggested);
                            move || id.clone()
                        })
                        .px_2()
                        .py_0p5()
                        .rounded(theme::RADIUS_MD)
                        .border_1()
                        .border_dashed()
                        .border_color(theme::border_strong())
                        .hover(|s| s.bg(theme::bg_control_hover()))
                        .cursor_pointer()
                        .text_xs()
                        .text_color(theme::text_muted())
                        .child(suggested.to_string())
                        .on_click(move |_e, _window, cx| {
                            if let Some(app) = app_suggest.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.batch_add_tag(tag_str.clone(), cx);
                                });
                            }
                        })
                }))
        }))
        .child(
            div()
                .id("open-tag-manager-link")
                .debug_selector(|| "open-tag-manager-link".to_string())
                .cursor_pointer()
                .text_xs()
                .text_color(theme::text_muted())
                .hover(|s| s.text_color(theme::text()))
                .child("Open Tag Manager…")
                .on_click(move |_e, _window, cx| {
                    if let Some(app) = app_manager.upgrade() {
                        app.update(cx, |this, cx| this.open_tag_manager(cx));
                    }
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::tag_suggestions;

    #[core::prelude::v1::test]
    fn suggestions_skip_present_tags_filter_by_prefix_and_cap_at_six() {
        let library: Vec<(String, usize)> = [
            "travel", "Trees", "beach", "trains", "tram", "tokyo", "toys", "tea", "tent",
        ]
        .iter()
        .map(|t| (t.to_string(), 1))
        .collect();
        let on_item = vec!["TRAVEL".to_string()];
        assert_eq!(
            tag_suggestions(&on_item, &library, "tr"),
            vec!["Trees", "trains", "tram"],
            "present tags are excluded; matching is a case-insensitive prefix"
        );
        assert_eq!(
            tag_suggestions(&on_item, &library, "").len(),
            6,
            "at most six"
        );
    }
}
