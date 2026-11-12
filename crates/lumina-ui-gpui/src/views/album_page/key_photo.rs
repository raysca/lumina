//! The album key photo (Design 07 Task 5): the panel's KEY PHOTO section,
//! its Change pick mode and the badge on the cover's grid card. The cover
//! shown is the effective one from the page's stats (the stored key photo,
//! else the newest member), read off the UI thread; nothing here queries
//! the catalog per frame.

use gpui::prelude::FluentBuilder;
use gpui::*;
use lumina_core::MediaId;

use crate::app::LuminaApp;
use crate::i18n::t;
use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::page_header::text_marker;

impl LuminaApp {
    /// The open album page's cover as last read: the badge and the panel
    /// both show it.
    pub(crate) fn album_page_cover(&self) -> Option<MediaId> {
        let id = self.album_page_album()?;
        self.album_page_stats(id)?.cover
    }

    /// Change is picking: the next clicked or ↵-ed item becomes the cover.
    pub(crate) fn cover_picking(&self) -> bool {
        self.albums.page.picking_cover.is_some()
            && self.albums.page.picking_cover == self.album_page_album()
    }

    /// Change in the panel: starts picking, or cancels a pick. Needs a
    /// cover to change (an empty album has none). The panel shows the hint
    /// while picking; Change is only in the panel, so no toast repeats it.
    pub(crate) fn toggle_cover_pick(&mut self, cx: &mut Context<Self>) {
        self.albums.page.picking_cover = if self.cover_picking() {
            None
        } else {
            self.album_page_album()
                .filter(|_| self.album_page_cover().is_some())
        };
        cx.notify();
    }

    /// Ends a pick with `item` as the cover (one undo step, as Set as
    /// Cover).
    pub(crate) fn pick_cover(&mut self, item: MediaId, cx: &mut Context<Self>) {
        self.albums.page.picking_cover = None;
        self.set_album_cover(item, cx);
    }

    /// A grid card's click while picking, or the second click of the
    /// double-click that picked: true when it is taken here.
    pub(crate) fn cover_pick_click(
        &mut self,
        item: MediaId,
        click_count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.cover_picking() {
            self.albums.page.picked_by_click = Some(item);
            self.pick_cover(item, cx);
            return true;
        }
        let picked = self.albums.page.picked_by_click.take();
        click_count >= 2 && picked == Some(item)
    }

    /// Pick mode keys: ↵ picks the selected item, Esc cancels. True when
    /// the key was taken.
    pub(crate) fn handle_cover_pick_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if !self.cover_picking() || self.is_fullscreen_viewer || self.context_menu_state.is_some() {
            return false;
        }
        match key {
            "enter" | "return" => {
                if let Some(item) = self.single_selected_id() {
                    self.pick_cover(item, cx);
                }
                true
            }
            "escape" => {
                self.albums.page.picking_cover = None;
                cx.notify();
                true
            }
            _ => false,
        }
    }
}

/// KEY PHOTO for album `id`: label and Change, the cover's thumbnail
/// (76×50), its file name and what it is for, or the pick hint.
pub(crate) fn section(this: &LuminaApp, id: i64, cx: &mut Context<LuminaApp>) -> Div {
    let cover = this.album_page_stats(id).and_then(|stats| stats.cover);
    let picking = this.cover_picking();
    let thumb =
        cover.and_then(|cover| crate::views::albums_overview::albums::thumbnail(this, cover));
    let name: SharedString = match cover {
        Some(_) => this.album_page_cover_name(id).unwrap_or_default(),
        None => t("album_panel.no_key_photo").into(),
    };
    let change: SharedString = t(if picking {
        "album_panel.change_cancel"
    } else {
        "album_panel.change"
    })
    .into();
    let sub: SharedString = t(if picking {
        "album_panel.pick_hint"
    } else {
        "album_panel.key_photo_sub"
    })
    .into();
    let enabled = cover.is_some();
    div()
        .debug_selector(|| "album-panel-key-photo".to_string())
        .flex()
        .flex_col()
        .gap(px(10.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(9.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::t3())
                        .child(t("album_panel.key_photo")),
                )
                .child(
                    div()
                        .id("album-panel-key-photo-change")
                        .debug_selector(|| "album-panel-key-photo-change".to_string())
                        .role(Role::Button)
                        .aria_label(change.clone())
                        .tooltip(theme::tooltip_label(t("album_panel.change_tooltip")))
                        .text_size(px(11.0))
                        .text_color(if enabled {
                            theme::accent()
                        } else {
                            theme::t3()
                        })
                        // GPUI has no disabled state: dimmed, no click,
                        // and the description says why.
                        .when(!enabled, |link| {
                            link.aria_description(t("album_panel.change_disabled"))
                        })
                        .when(enabled, |link| {
                            link.cursor_pointer()
                                .hover(|s| s.text_color(theme::t1()))
                                .on_click(cx.listener(|this, _e: &ClickEvent, _w, cx| {
                                    this.toggle_cover_pick(cx);
                                }))
                        })
                        .child(change),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .debug_selector(|| "album-panel-key-photo-thumb".to_string())
                        .flex_none()
                        .w(px(76.0))
                        .h(px(50.0))
                        .rounded(px(5.0))
                        .overflow_hidden()
                        .bg(theme::bg_2())
                        .when(picking, |well| {
                            well.border_1().border_color(theme::accent())
                        })
                        .children(
                            thumb.map(|texture| {
                                img(texture).size_full().object_fit(ObjectFit::Cover)
                            }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .debug_selector(|| "album-panel-key-photo-name".to_string())
                                .truncate()
                                .font_family(theme::FONT_MONO)
                                .text_size(px(11.0))
                                .text_color(if enabled { theme::t1() } else { theme::t3() })
                                .children(text_marker("album-panel-key-photo-name", &name))
                                .child(name),
                        )
                        .child(
                            div()
                                .when(picking, |hint| {
                                    hint.debug_selector(|| "album-panel-key-photo-hint".to_string())
                                })
                                .text_size(px(10.5))
                                .text_color(if picking {
                                    theme::accent()
                                } else {
                                    theme::t3()
                                })
                                .when(enabled, |line| line.child(sub)),
                        ),
                ),
        )
}

/// The key photo badge on the cover's grid card: star and "KEY PHOTO" in
/// mono accent on a dark pill, radius 4, 19 high.
pub(crate) fn badge(item_id: u64) -> Stateful<Div> {
    let label: SharedString = t("album_panel.key_photo").into();
    div()
        .id(("key-photo-badge", item_id))
        .debug_selector(move || format!("key-photo-badge-{item_id}"))
        .role(Role::Image)
        .aria_label(label.clone())
        .h(px(19.0))
        .px(px(7.0))
        .rounded(px(4.0))
        .bg(theme::key_photo_scrim())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .font_family(theme::FONT_MONO)
        .text_size(px(8.5))
        .text_color(theme::accent())
        .child(render_icon(Icon::StarFilled, px(9.0), theme::accent()))
        .child(label)
}
