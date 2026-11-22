use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::{MediaItem, MediaType};

#[cfg(test)]
use crate::app::types::format_bytes;
use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::popover_under;

/// Viewer top bar height: the title bar band, so Back and the title centre
/// on the window controls. The floating immersive info panel starts below it.
pub(super) const TOP_BAR_HEIGHT: f32 = theme::TITLEBAR_HEIGHT;
/// Below this stage width the Back button drops its label.
const COMPACT_STAGE_WIDTH: f32 = 760.0;
const BUTTON: f32 = 30.0;

pub struct ViewerTopBar;

/// "23 / 84": the item's place in the current view.
pub(crate) fn position_label(index: usize, count: usize) -> String {
    format!("{} / {}", index + 1, count)
}

/// The video's display title, after Back. Only the title: the date and
/// resolution live in the inspector, which keeps the bar quiet.
fn video_title_block(item: &MediaItem) -> Stateful<Div> {
    div()
        .id("viewer-title")
        .debug_selector(|| "viewer-title".to_string())
        .min_w_0()
        .truncate()
        .font_family(theme::FONT_DISPLAY)
        .text_size(px(15.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme::t1())
        .child(crate::views::inspector::body::display_title(item))
}

/// A 30x30 icon button with a tooltip and an accessible label; `on` shows
/// the active fill.
fn bar_button(id: &'static str, label: String, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.to_string())
        .role(Role::Button)
        .aria_label(label.clone())
        .tooltip(theme::tooltip_label(label))
        .flex_none()
        .size(px(BUTTON))
        .rounded(theme::RADIUS_LG)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .when(on, |b| b.bg(theme::bg_2()))
        .hover(|s| s.bg(theme::bg_3()))
}

fn bar_icon(icon: Icon, on: bool) -> Svg {
    render_icon(icon, px(16.0), if on { theme::t1() } else { theme::t2() })
}

/// The star popover's card: five stars (filled up to the rating) and the
/// key hint. It occludes, so clicks on it never reach the dismiss backdrop.
fn rate_card(rating: u8, cx: &mut Context<LuminaApp>) -> Div {
    div()
        .debug_selector(|| "viewer-rate-popover".to_string())
        .occlude()
        .p_2()
        .bg(theme::bg_1())
        .border_1()
        .border_color(theme::line())
        .rounded(theme::RADIUS_LG)
        .shadow_2xl()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .child(
            div()
                .flex()
                .flex_row()
                .gap_0p5()
                .children((1..=5u8).map(|n| {
                    let id = format!("viewer-rate-star-{n}");
                    let filled = n <= rating;
                    div()
                        .id(SharedString::from(id.clone()))
                        .debug_selector(move || id.clone())
                        .role(Role::Button)
                        .aria_label(t(&format!("command.rate_{n}")))
                        .size(px(28.0))
                        .rounded(theme::RADIUS_MD)
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme::bg_3()))
                        .child(render_icon(
                            if filled { Icon::StarFilled } else { Icon::Star },
                            px(16.0),
                            if filled {
                                theme::rating()
                            } else {
                                theme::rating_unset()
                            },
                        ))
                        .on_click(cx.listener(move |this, _e: &ClickEvent, _w, cx| {
                            this.rate_from_popover(n, cx);
                        }))
                })),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::t3())
                .child(t("viewer.rate_hint")),
        )
}

impl ViewerTopBar {
    pub fn render(
        this: &LuminaApp,
        item: &MediaItem,
        cx: &mut Context<LuminaApp>,
    ) -> impl IntoElement {
        let is_inspector_open = this.inspector_open;
        let compact = super::stage_width(this).is_some_and(|w| w < COMPACT_STAGE_WIDTH);
        let is_fav = item.is_favorite;
        let view_label = this.view_label();
        let rate_open = this.rate_popover_shown();
        let is_video = item.media_type == MediaType::Video;

        // "23 / 84" in the current view; it truncates first in a narrow
        // stage. Hidden when the viewer has no grid index, and for a video,
        // whose title takes the left instead ([ and ] still step).
        let center_section = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .px_2()
            .children(this.selected_index.filter(|_| !is_video).map(|index| {
                div()
                    .id("viewer-position")
                    .debug_selector(|| "viewer-position".to_string())
                    .min_w_0()
                    .truncate()
                    .font_family(theme::FONT_MONO)
                    .text_size(px(11.0))
                    .text_color(theme::t3())
                    .child(position_label(index, this.displayed_count()))
            }));

        let back_tooltip = t("viewer.back_tooltip").replace("{view}", &view_label);
        let back = div()
            .id("viewer-back")
            .debug_selector(|| "viewer-back".to_string())
            .role(Role::Button)
            .aria_label(back_tooltip.clone())
            .tooltip(theme::tooltip_label(back_tooltip))
            .h(px(BUTTON))
            .pl(px(6.0))
            .pr(px(10.0))
            .bg(theme::bg_2())
            .hover(|s| s.bg(theme::bg_3()))
            .rounded(theme::RADIUS_LG)
            .cursor_pointer()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(render_icon(Icon::ChevronLeft, px(15.0), theme::t2()))
            .when(!compact, |back| {
                back.child(
                    div()
                        .debug_selector(|| "viewer-back-label".to_string())
                        .max_w(px(220.0))
                        .truncate()
                        .text_size(px(12.0))
                        .text_color(theme::t2())
                        .child(view_label),
                )
            })
            .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                // During a slideshow Back is Esc: it also puts the
                // selection back.
                if this.slideshow.is_some() {
                    this.end_slideshow(cx);
                } else {
                    this.close_viewer(cx);
                }
            }));

        // Edit opens edit mode; videos cannot be edited.
        let can_edit = item.media_type != MediaType::Video;

        div()
            .id("viewer-top-bar")
            .debug_selector(|| "viewer-top-bar".to_string())
            .on_hover(cx.listener(|this, on: &bool, _w, _cx| this.set_pointer_on_hud(*on)))
            .w_full()
            .flex_none()
            .h(px(TOP_BAR_HEIGHT))
            .px(px(20.0))
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .text_xs()
            .text_color(theme::t1())
            // Left: the macOS traffic lights sit here while windowed, then
            // the single Back control (Escape stays equivalent).
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .min_w_0()
                    .when(!is_video, |left| left.flex_shrink_0())
                    .when(!this.is_immersive_fullscreen, |left| {
                        left.child(div().flex_none().w(px(64.0)))
                    })
                    .child(back.flex_none())
                    // 14px from Back (gap_3 is 12).
                    .when(is_video, |left| {
                        left.child(video_title_block(item).ml(px(2.0)))
                    }),
            )
            .child(center_section)
            // Right: Favourite, Rate, Edit, Share, Info. A slideshow has
            // none: they would act on, or cover, a moving slide. Open,
            // Reveal and Rotate live in the right-click menu.
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .flex_shrink_0()
                    .when(this.slideshow.is_none(), |right| {
                        right
                            .child(
                                bar_button(
                                    "viewer-fav-btn",
                                    t(if is_fav {
                                        "viewer.unfavorite_tooltip"
                                    } else {
                                        "viewer.favorite_tooltip"
                                    }),
                                    is_fav,
                                )
                                .child(crate::motion::pop(
                                    render_icon(
                                        if is_fav {
                                            Icon::HeartFilled
                                        } else {
                                            Icon::Heart
                                        },
                                        px(16.0),
                                        if is_fav { theme::rating() } else { theme::t2() },
                                    ),
                                    this.mark_pop.heart(Some(item.id)),
                                ))
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_favorite_selected(cx);
                                    },
                                )),
                            )
                            .child(
                                bar_button("viewer-rate-btn", t("viewer.rate_tooltip"), rate_open)
                                    .relative()
                                    .child(bar_icon(Icon::Star, rate_open))
                                    .when(rate_open, |button| {
                                        let rating =
                                            this.item_ratings.get(&item.id).copied().unwrap_or(0);
                                        button.child(popover_under(
                                            Anchor::TopRight,
                                            rate_card(rating, cx),
                                        ))
                                    })
                                    .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_rate_popover(cx);
                                    })),
                            )
                            .when(can_edit, |right| {
                                right.child(
                                    bar_button("viewer-edit-btn", t("viewer.edit_tooltip"), false)
                                        .child(bar_icon(Icon::WandSparkles, false))
                                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                            this.enter_edit_mode(cx);
                                        })),
                                )
                            })
                            .children(share_button(cx))
                            .when(is_video, |right| {
                                right.child(
                                    bar_button("viewer-pip-btn", t("viewer.pip_tooltip"), false)
                                        .child(bar_icon(Icon::Pip, false))
                                        .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                            if let Some(item) = this.selected_item.clone() {
                                                this.play_in_pip(item, cx);
                                            }
                                        })),
                                )
                            })
                            .child(
                                bar_button(
                                    "viewer-info-btn",
                                    t("viewer.info_tooltip"),
                                    is_inspector_open,
                                )
                                .child(bar_icon(Icon::PanelRight, is_inspector_open))
                                .on_click(cx.listener(
                                    |this, _e: &ClickEvent, _w, cx| {
                                        this.toggle_inspector(cx);
                                    },
                                )),
                            )
                    }),
            )
    }
}

/// Share… (macOS only, like the batch dock), anchored where it was clicked.
#[cfg(target_os = "macos")]
fn share_button(cx: &mut Context<LuminaApp>) -> Option<Stateful<Div>> {
    Some(
        bar_button("viewer-share-btn", t("viewer.share_tooltip"), false)
            .child(bar_icon(Icon::Share, false))
            .on_click(cx.listener(|this, e: &ClickEvent, _w, cx| {
                this.share_selection(Some(e.position()), cx);
            })),
    )
}

#[cfg(not(target_os = "macos"))]
fn share_button(_cx: &mut Context<LuminaApp>) -> Option<Stateful<Div>> {
    None
}

/// "4032 × 3024 · HEIC · 3.1 MB" from what the catalog actually knows: no
/// guessed codec, frame rate or dimensions.
#[cfg(test)]
pub(crate) fn specs_line(item: &lumina_core::MediaItem) -> String {
    let mut parts = Vec::new();
    if item.width > 0 && item.height > 0 {
        parts.push(format!("{} × {}", item.width, item.height));
    }
    if let Some(ext) = item.path.extension().and_then(|e| e.to_str()) {
        parts.push(ext.to_uppercase());
    }
    parts.push(format_bytes(item.file_size));
    parts.join(" · ")
}
