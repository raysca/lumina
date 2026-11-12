//! The album view's inline selection bar (Design 07 Task 2): a band under
//! the header that replaces the floating dock while an album page has a
//! selection. Favorite is not here; F and the palette keep it.

use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::app::LuminaApp;
use crate::i18n::{count, t};
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

/// Height of the bar.
const SELECTION_BAR_HEIGHT: f32 = 55.5;
/// Below this main-column width the buttons drop their labels: about a
/// 1100 px window beside the default sidebar.
const COMPACT_BELOW: f32 = 860.0;

impl LuminaApp {
    /// The bar shows: an album page with a selection.
    pub(crate) fn album_selection_bar_shown(&self) -> bool {
        self.album_page_album().is_some() && self.has_selection()
    }
}

/// A bar button's look.
#[derive(Clone, Copy, PartialEq)]
enum Look {
    /// `bg_2` fill.
    Filled,
    /// Transparent until hovered (Remove).
    Ghost,
    /// `danger_soft` fill, `danger` text, `danger_border` outline (Delete).
    Danger,
}

/// A 32-high bar button: radius 8, 13px icon and Inter 12. Icon-only and
/// 32 wide when `compact`; the tooltip always names the action.
fn bar_button(
    selector: &'static str,
    icon: Icon,
    label: &str,
    tooltip: String,
    look: Look,
    compact: bool,
) -> Stateful<Div> {
    let label: SharedString = t(label).into();
    let (fill, hover, color) = match look {
        Look::Filled => (theme::bg_2(), theme::bg_3(), theme::t1()),
        Look::Ghost => (gpui::transparent_black().into(), theme::bg_2(), theme::t1()),
        Look::Danger => (
            theme::danger_soft(),
            theme::danger_bg_hover(),
            theme::danger(),
        ),
    };
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(tooltip))
        .h(px(32.0))
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .map(|b| {
            if compact {
                b.w(px(32.0))
            } else {
                b.px(px(11.0))
            }
        })
        .rounded(theme::RADIUS_LG)
        .bg(fill)
        .hover(move |s| s.bg(hover))
        .when(look == Look::Danger, |b| {
            b.border_1().border_color(theme::danger_border())
        })
        .cursor_pointer()
        .text_size(px(12.0))
        .text_color(color)
        .child(render_icon(icon, px(13.0), color))
        .when(!compact, |b| b.child(label))
}

/// A text link on the left: Select all and Clear.
fn link(selector: &'static str, text: SharedString, tooltip: String, color: Rgba) -> Stateful<Div> {
    div()
        .id(selector)
        .debug_selector(move || selector.to_string())
        .role(Role::Button)
        .aria_label(text.clone())
        .tooltip(theme::tooltip_label(tooltip))
        .flex_shrink_0()
        .cursor_pointer()
        .text_size(px(11.5))
        .text_color(color)
        .hover(|s| s.text_color(theme::t1()))
        .children(text_marker(selector, &text))
        .child(text)
}

fn divider(height: f32) -> Div {
    div()
        .w(px(1.0))
        .h(px(height))
        .flex_shrink_0()
        .bg(theme::line_2())
}

/// The bar, or nothing without an album page selection.
pub(crate) fn render(this: &LuminaApp, window: &Window, cx: &mut Context<LuminaApp>) -> AnyElement {
    if !this.album_selection_bar_shown() {
        return Empty.into_any_element();
    }
    let sidebar = if this.sidebar_collapsed {
        crate::views::grid::SIDEBAR_COLLAPSED_WIDTH
    } else {
        this.sidebar_width
    };
    let compact = f32::from(window.viewport_size().width) - sidebar < COMPACT_BELOW;
    let selected: SharedString = t("album_bar.selected")
        .replace("{n}", &count(this.batch_summary.count))
        .into();
    let shown = this.displayed_count();
    let select_all: SharedString = t("album_bar.select_all")
        .replace("{n}", &count(shown))
        .into();

    let left = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .child(render_icon(Icon::CircleCheckBig, px(16.0), theme::accent()).flex_shrink_0())
        .child(
            div()
                .debug_selector(|| "album-bar-count".to_string())
                .flex_shrink_0()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::t1())
                .children(text_marker("album-bar-count", &selected))
                .child(selected),
        )
        .child(divider(18.0))
        .child(
            link(
                "album-select-all",
                select_all,
                t("album_bar.select_all_tooltip"),
                theme::accent(),
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.select_all(cx))),
        )
        .child(
            link(
                "album-bar-clear",
                t("album_bar.clear").into(),
                t("album_bar.clear_tooltip"),
                theme::t3(),
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.clear_selection(cx))),
        );

    let right = div()
        .flex_shrink_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .child(
            bar_button(
                "album-bar-add",
                Icon::FolderPlus,
                "album_bar.add",
                t("album_bar.add_tooltip"),
                Look::Filled,
                compact,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.open_album_picker(cx))),
        )
        .child(
            bar_button(
                "album-bar-tag",
                Icon::Hash,
                "album_bar.tag",
                t("album_bar.tag_tooltip"),
                Look::Filled,
                compact,
            )
            .on_click(
                cx.listener(|this, _e: &ClickEvent, _w, cx| this.begin_tagging_selection(cx)),
            ),
        )
        .child({
            // Exactly one item: a cover is one photo.
            let one = this.single_selected_id().is_some();
            bar_button(
                "album-bar-set-cover",
                Icon::ImagePlus,
                "album_bar.set_cover",
                t(if one {
                    "album_bar.set_cover_tooltip"
                } else {
                    "album_bar.set_cover_disabled"
                }),
                Look::Filled,
                compact,
            )
            .map(|button| {
                if one {
                    button.on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                        this.set_album_cover_from_selection(cx)
                    }))
                } else {
                    // GPUI has no disabled state: dimmed, no click, and
                    // the tooltip says why.
                    button
                        .aria_description(t("album_bar.set_cover_disabled"))
                        .opacity(0.4)
                        .cursor_default()
                }
            })
        })
        .child(
            bar_button(
                "album-bar-export",
                Icon::Download,
                "album_bar.export",
                t("album_bar.export_tooltip"),
                Look::Filled,
                compact,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                this.export_pick_folder(lumina_core::ExportPreset::Originals, cx)
            })),
        )
        .child(divider(20.0))
        .child(
            bar_button(
                "album-bar-remove",
                Icon::FolderMinus,
                "album_bar.remove",
                t("album_bar.remove_tooltip"),
                Look::Ghost,
                compact,
            )
            .on_click(
                cx.listener(|this, _e: &ClickEvent, _w, cx| this.remove_selection_from_album(cx)),
            ),
        )
        .child(
            bar_button(
                "album-bar-delete",
                Icon::Trash,
                "album_bar.delete",
                t("album_bar.delete_tooltip"),
                Look::Danger,
                compact,
            )
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| this.prompt_delete_selected(cx))),
        );

    div()
        .id("album-selection-bar")
        .debug_selector(|| "album-selection-bar".to_string())
        .role(Role::Toolbar)
        .aria_label(t("album_bar.label"))
        // Clicks on the bar must not reach the grid, which clears the
        // selection the bar acts on.
        .occlude()
        .w_full()
        .h(px(SELECTION_BAR_HEIGHT))
        .flex_shrink_0()
        .px(px(28.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(16.0))
        .bg(theme::accent_surface())
        .border_b_1()
        .border_color(theme::warn_border())
        .child(left)
        .child(right)
        .into_any_element()
}
