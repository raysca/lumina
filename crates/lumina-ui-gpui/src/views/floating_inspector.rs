use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::render_inspector_body;
use crate::views::sidebar::AlbumSelection;
use crate::{LuminaApp, UiMessage};

pub struct FloatingInspectorView {
    pub app: WeakEntity<LuminaApp>,
    pub focus_handle: FocusHandle,
    pub ui_tx: async_channel::Sender<UiMessage>,
    pub _subscription: Option<Subscription>,
}

impl FloatingInspectorView {
    pub fn new(
        app: WeakEntity<LuminaApp>,
        ui_tx: async_channel::Sender<UiMessage>,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        let _subscription = app.upgrade().map(|app_entity| {
            cx.observe(&app_entity, |_this, _entity, cx| {
                cx.notify();
            })
        });
        Self {
            app,
            focus_handle,
            ui_tx,
            _subscription,
        }
    }
}

impl Drop for FloatingInspectorView {
    fn drop(&mut self) {
        let _ = self.ui_tx.send_blocking(UiMessage::FloatingInspectorClosed);
    }
}

impl Render for FloatingInspectorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = match self.app.upgrade() {
            Some(app) => app,
            None => {
                return div()
                    .size_full()
                    .bg(theme::bg_pane())
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_sm()
                    .text_color(theme::text_subtle())
                    .child("Main library window closed")
                    .into_any_element();
            }
        };

        // This window paints `viewer_image` too; evict retired frames here
        // so playback cannot pile them up while the main window is hidden.
        let retired = app.update(cx, |app, _| std::mem::take(&mut app.retired_images));
        crate::app::viewer_actions::evict_retired_images(retired, window, cx);

        let app_clone = self.app.clone();
        let app_redock = self.app.clone();
        let app_read = app.read(cx);

        let selected_indices_len = app_read.selected_indices.len();
        let selected_item = app_read.selected_item.clone();
        let viewer_image = app_read.viewer_image.clone();
        let batch_summary = app_read.batch_summary.clone();
        let selected_item_tags = app_read.selected_item_tags.clone();
        let library_tag_counts = app_read.library_tag_counts.clone();
        let tag_input_active = app_read.tag_input_active;
        let tag_input_buffer = app_read.tag_input_buffer.clone();
        let is_trash = app_read.selection == AlbumSelection::Trash;
        let tab = app_read.inspector_tab;
        let rating = selected_item
            .as_ref()
            .map(|it| app_read.get_item_rating(it))
            .unwrap_or(0);
        let color_label = selected_item
            .as_ref()
            .and_then(|it| app_read.item_color_labels.get(&it.id).copied());
        let caption = selected_item
            .as_ref()
            .and_then(|it| app_read.item_captions.get(&it.id).cloned());
        let caption_input_active = app_read.caption.input_active;
        let caption_input_buffer = app_read.caption.input_buffer.clone();
        let title_input_active = app_read.caption.title_input_active;
        let title_input_buffer = app_read.caption.title_input_buffer.clone();
        let date_input_active = app_read.caption.date_input_active;
        let date_input_buffer = app_read.caption.date_input_buffer.clone();
        let location_input_active = app_read.caption.location_input_active;
        let location_input_buffer = app_read.caption.location_input_buffer.clone();

        let file_name = selected_item
            .as_ref()
            .map(|i| i.file_name.clone())
            .unwrap_or_else(|| "Media".to_string());
        // The title is one unbreakable line in a fixed-height bar: truncate it
        // and keep the full text in a tooltip so a long name can never push
        // the Dock/Close controls out of the window (BUG 12).
        let window_title = if selected_indices_len > 1 {
            format!("{} items — Info", selected_indices_len)
        } else {
            format!("{} — Info", file_name)
        };

        let app_key_event = self.app.clone();
        let app_traffic_close = self.app.clone();

        div()
            .id("floating-inspector-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(move |_this, event: &KeyDownEvent, window, cx| {
                if let Some(app) = app_key_event.upgrade() {
                    app.update(cx, |this, cx| {
                        this.handle_key_down(event, window, cx);
                    });
                }
            }))
            .size_full()
            .bg(theme::bg_pane())
            .text_color(theme::text())
            .flex()
            .flex_col()
            .overflow_hidden()
            // 1. Draggable macOS Titlebar with Traffic Lights & Window Title
            .child(
                div()
                    .id("floating-inspector-titlebar")
                    .w_full()
                    .h(px(38.0))
                    .px_4()
                    .bg(theme::bg_pane())
                    .border_b_1()
                    .border_color(theme::border_subtle())
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .on_mouse_down(MouseButton::Left, |_e, window, _cx| {
                        window.start_window_move();
                    })
                    // Left: Traffic Light dots
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            // Close (red)
                            .child(
                                div()
                                    .id("traffic-light-close")
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(theme::label_red())
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |_this, _e: &ClickEvent, window, cx| {
                                            if let Some(app_entity) = app_traffic_close.upgrade() {
                                                app_entity.update(cx, |app, cx| {
                                                    app.floating_inspector_closed(cx);
                                                });
                                            }
                                            window.remove_window();
                                        },
                                    )),
                            )
                            // Minimize (yellow)
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(theme::label_yellow()),
                            )
                            // Zoom (green)
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(theme::label_green()),
                            ),
                    )
                    // Center: Window Title
                    .child(
                        div()
                            .id("floating-inspector-title")
                            .min_w(px(0.0))
                            .truncate()
                            .tooltip(theme::tooltip_label(window_title.clone()))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme::text())
                            .child(window_title),
                    )
                    // Right: Actions (Dock & Close)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            // Dock button
                            .child(
                                div()
                                    .id("floating-inspector-redock-btn")
                                    .px_2()
                                    .py_0p5()
                                    .bg(theme::bg_raised())
                                    .hover(|s| s.bg(theme::bg_control_hover()))
                                    .border_1()
                                    .border_color(theme::border())
                                    .rounded_md()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text_muted())
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_1()
                                    .child(render_icon(
                                        Icon::ArrowDownToLine,
                                        px(11.0),
                                        theme::text_muted(),
                                    ))
                                    .child("Dock")
                                    .on_click(cx.listener(
                                        move |_this, _e: &ClickEvent, window, cx| {
                                            if let Some(app_entity) = app_redock.upgrade() {
                                                app_entity.update(cx, |app, cx| {
                                                    app.redock_inspector(cx);
                                                });
                                            }
                                            window.remove_window();
                                        },
                                    )),
                            )
                            // Close button
                            .child(
                                div()
                                    .id("floating-inspector-close-btn")
                                    .tooltip(theme::tooltip_label("Close inspector"))
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .rounded_md()
                                    .hover(|s| s.bg(theme::bg_raised()))
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::text_subtle())
                                    .child(render_icon(Icon::X, px(11.0), theme::text_subtle()))
                                    .on_click(cx.listener(
                                        move |_this, _e: &ClickEvent, window, cx| {
                                            if let Some(app_entity) = app_clone.upgrade() {
                                                app_entity.update(cx, |app, cx| {
                                                    app.floating_inspector_closed(cx);
                                                });
                                            }
                                            window.remove_window();
                                        },
                                    )),
                            ),
                    ),
            )
            // 2. Revised Inspector Body (fully scrollable, clean layout)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(render_inspector_body(
                        self.app.clone(),
                        selected_item,
                        selected_indices_len,
                        &batch_summary,
                        viewer_image,
                        &selected_item_tags,
                        &library_tag_counts,
                        tag_input_active,
                        &tag_input_buffer,
                        is_trash,
                        tab,
                        rating,
                        color_label,
                        caption.as_deref(),
                        caption_input_active,
                        &caption_input_buffer,
                        title_input_active,
                        &title_input_buffer,
                        date_input_active,
                        &date_input_buffer,
                        location_input_active,
                        &location_input_buffer,
                    )),
            )
            .into_any_element()
    }
}
