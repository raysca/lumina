use crate::app::text_input::FieldEditor;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::LuminaApp;
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Library tags offered under the input; more would crowd the editor.
const MAX_SUGGESTIONS: usize = 6;

pub fn render_tags_tab(
    weak_app: WeakEntity<LuminaApp>,
    tags: &[String],
    suggestions: &[Suggestion],
    suggestion_focus: Option<usize>,
    tag_input_active: bool,
    field_input: Option<FieldEditor>,
) -> AnyElement {
    render_tags_section(
        weak_app,
        "ON THIS ITEM",
        tags,
        suggestions,
        suggestion_focus,
        tag_input_active,
        field_input,
    )
}

/// A keyword offered under the tag field: its stored path, and the alias
/// the typed text matched when it came through one.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Suggestion {
    pub path: String,
    pub via_alias: Option<String>,
}

impl Suggestion {
    /// `places › usa › new york`, then `(nyc)` when an alias matched.
    pub(crate) fn label(&self) -> String {
        let path = self.path.split('/').collect::<Vec<_>>().join(" › ");
        match &self.via_alias {
            Some(alias) => format!("{path} ({alias})"),
            None => path,
        }
    }
}

/// Up to six library keywords not already present, most used first, whose
/// whole path or any segment starts with the typed text (case-insensitive),
/// or that an alias starting with it points at. `counts` is the library's
/// tag counts, most used first; `aliases` its `(alias, keyword)` rows.
pub(crate) fn tag_suggestions(
    present: &[String],
    counts: &[(String, usize)],
    aliases: &[(String, String)],
    typed: &str,
) -> Vec<Suggestion> {
    let typed = typed.trim().to_lowercase();
    let is_present = |path: &str| present.iter().any(|t| t.eq_ignore_ascii_case(path));
    let matches = |path: &str| {
        let path = path.to_lowercase();
        path.starts_with(&typed) || path.split('/').any(|seg| seg.trim().starts_with(&typed))
    };
    // An empty field suggests the most used keywords, never via an alias.
    let alias_for = |path: &str| {
        aliases
            .iter()
            .find(|(alias, target)| {
                !typed.is_empty() && target == path && alias.starts_with(&typed)
            })
            .map(|(alias, _)| alias.clone())
    };
    let mut out: Vec<Suggestion> = Vec::new();
    // ponytail: a linear alias scan per unmatched keyword; index aliases by
    // keyword if libraries grow thousands of both.
    for (path, _) in counts {
        if out.len() == MAX_SUGGESTIONS {
            return out;
        }
        if is_present(path) {
            continue;
        }
        if matches(path) {
            out.push(Suggestion {
                path: path.clone(),
                via_alias: None,
            });
        } else if let Some(alias) = alias_for(path) {
            out.push(Suggestion {
                path: path.clone(),
                via_alias: Some(alias),
            });
        }
    }
    // An alias can name a keyword no item carries yet.
    for (alias, target) in aliases {
        if out.len() == MAX_SUGGESTIONS {
            break;
        }
        if !typed.is_empty()
            && alias.starts_with(&typed)
            && !is_present(target)
            && !out.iter().any(|s| s.path == *target)
        {
            out.push(Suggestion {
                path: target.clone(),
                via_alias: Some(alias.clone()),
            });
        }
    }
    out
}

/// Dashed suggestion chips under a tag field; `focus` is the one ↑/↓
/// highlighted (Enter applies it). Clicking applies the stored keyword.
pub(crate) fn render_suggestions(
    weak_app: WeakEntity<LuminaApp>,
    suggestions: &[Suggestion],
    focus: Option<usize>,
) -> Option<Div> {
    if suggestions.is_empty() {
        return None;
    }
    Some(
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap_1()
            .children(suggestions.iter().enumerate().map(|(i, suggestion)| {
                let path = suggestion.path.clone();
                let label = suggestion.label();
                let selector = format!("tag-suggestion-{path}");
                let app = weak_app.clone();
                let focused = focus == Some(i);
                div()
                    .id(ElementId::Name(selector.clone().into()))
                    .debug_selector(move || selector.clone())
                    .role(Role::Button)
                    .aria_label(format!("Add tag {label}"))
                    .px_2()
                    .py_0p5()
                    .rounded(theme::RADIUS_MD)
                    .border_1()
                    .border_dashed()
                    .border_color(if focused {
                        theme::focus_ring()
                    } else {
                        theme::border_strong()
                    })
                    .when(focused, |chip| chip.bg(theme::bg_control_hover()))
                    .hover(|s| s.bg(theme::bg_control_hover()))
                    .cursor_pointer()
                    .text_xs()
                    .text_color(theme::text_muted())
                    .child(label)
                    .on_click(move |_e, _window, cx| {
                        if let Some(app) = app.upgrade() {
                            app.update(cx, |this, cx| this.batch_add_tag(path.clone(), cx));
                        }
                    })
            })),
    )
}

/// The tag editor (v3): chips on the item, an always-visible ADD field where
/// Enter adds, dashed suggestion chips, and a link to the Tag Manager.
pub fn render_tags_section(
    weak_app: WeakEntity<LuminaApp>,
    heading: &'static str,
    tags: &[String],
    suggestions: &[Suggestion],
    suggestion_focus: Option<usize>,
    tag_input_active: bool,
    field_input: Option<FieldEditor>,
) -> AnyElement {
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
                    Some(input) => div().flex_1().text_color(theme::text()).child(input),
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
        .children(render_suggestions(
            weak_app.clone(),
            suggestions,
            suggestion_focus,
        ))
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

    fn paths(suggestions: Vec<super::Suggestion>) -> Vec<String> {
        suggestions.into_iter().map(|s| s.path).collect()
    }

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
            paths(tag_suggestions(&on_item, &library, &[], "tr")),
            vec!["Trees", "trains", "tram"],
            "present tags are excluded; matching is a case-insensitive prefix"
        );
        assert_eq!(
            tag_suggestions(&on_item, &library, &[], "").len(),
            6,
            "at most six"
        );
    }

    #[core::prelude::v1::test]
    fn suggestions_match_segments_and_aliases_with_a_path_label() {
        let library: Vec<(String, usize)> = vec![
            ("places/usa/new york".into(), 3),
            ("places/europe/paris".into(), 2),
            ("newsletter".into(), 1),
        ];
        let aliases = vec![
            ("nyc".to_string(), "places/usa/new york".to_string()),
            ("lisbon".to_string(), "places/europe/lisbon".to_string()),
        ];
        assert_eq!(
            paths(tag_suggestions(&[], &library, &aliases, "new")),
            ["places/usa/new york", "newsletter"],
            "a segment prefix matches, most used first"
        );
        assert_eq!(
            paths(tag_suggestions(&[], &library, &aliases, "places/eu")),
            ["places/europe/paris"],
            "a whole-path prefix matches"
        );
        let via = tag_suggestions(&[], &library, &aliases, "ny");
        assert_eq!(via[0].label(), "places › usa › new york (nyc)");
        assert_eq!(
            paths(tag_suggestions(&[], &library, &aliases, "lis")),
            ["places/europe/lisbon"],
            "an alias may name a keyword no item carries yet"
        );
        assert!(
            tag_suggestions(&[], &library, &aliases, "")
                .iter()
                .all(|s| s.via_alias.is_none()),
            "an empty field never suggests through an alias"
        );
    }
}
