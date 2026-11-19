use gpui::*;

use crate::icons::{render_icon, Icon};
use crate::theme;
use crate::views::inspector::{render_inspector_body, InspectorBody};
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        // Keys reach the app through this handle, even when the window
        // becomes key without a click (⌘`).
        window.focus(&focus_handle, cx);
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
                    .bg(theme::bg_1())
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_sm()
                    .text_color(theme::t2())
                    .child("Main library window closed")
                    .into_any_element();
            }
        };

        // This window paints `viewer_image` too; evict retired frames here
        // so playback cannot pile them up while the main window is hidden.
        let root = self.focus_handle.clone();
        let retired = app.update(cx, |app, cx| {
            app.sync_field_input(window, &root, true, cx);
            app.ensure_info_details(cx);
            std::mem::take(&mut app.retired_images)
        });
        crate::app::viewer_actions::evict_retired_images(retired, window, cx);

        let app_redock = self.app.clone();
        let app_read = app.read(cx);

        let selected_indices_len = app_read.selected_indices.len();
        // A multi-selection gets the same v3 batch panel as the docked one.
        let batch = (selected_indices_len > 1)
            .then(|| crate::views::inspector::batch::render_batch(app_read, self.app.clone()));
        let selected_item = app_read.selected_item.clone();
        let viewer_image = app_read.viewer_image.clone();
        let selected_item_tags = app_read.selected_item_tags.clone();
        let info_details = app_read.info_details.clone();
        let suggestions = app_read.tag_input_suggestions();
        let suggestion_focus = app_read.keywords.suggestion_focus;
        let tag_input_active = app_read.tag_input_active;
        let is_trash = app_read.selection == AlbumSelection::Trash;
        let tab = app_read.inspector_tab;
        let rating = selected_item
            .as_ref()
            .map(|it| app_read.get_item_rating(it))
            .unwrap_or(0);
        let mark_pop = app_read.mark_pop.clone();
        let caption = selected_item
            .as_ref()
            .and_then(|it| app_read.item_captions.get(&it.id).cloned());
        let caption_input_active = app_read.caption.input_active;
        let title_input_active = app_read.caption.title_input_active;
        let date_input_active = app_read.caption.date_input_active;
        let location_input_active = app_read.caption.location_input_active;
        // The inspector's fields all share the one field input.
        let field_input = app_read
            .text_target()
            .and_then(|target| app_read.field_input_for(target));

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

        crate::app::text_input::field_input_actions(div(), &app)
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
            .bg(theme::bg_1())
            .text_color(theme::t1())
            .flex()
            .flex_col()
            .overflow_hidden()
            // 1. Draggable titlebar; the system draws the traffic lights.
            .child(
                div()
                    .id("floating-inspector-titlebar")
                    .debug_selector(|| "floating-inspector-titlebar".to_string())
                    .relative()
                    .w_full()
                    .h(px(38.0))
                    .px_4()
                    .bg(theme::bg_1())
                    .border_b_1()
                    .border_color(theme::line())
                    .flex()
                    .flex_row()
                    .justify_end()
                    .items_center()
                    .on_mouse_down(MouseButton::Left, |_e, window, _cx| {
                        window.start_window_move();
                    })
                    // Center: window title, centred on the bar (v3: 13px medium).
                    .child(
                        div()
                            .absolute()
                            .left(px(120.0))
                            .right(px(120.0))
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .id("floating-inspector-title")
                                    .debug_selector(|| "floating-inspector-title".to_string())
                                    .min_w(px(0.0))
                                    .truncate()
                                    .tooltip(theme::tooltip_label(window_title.clone()))
                                    .text_size(theme::TEXT_BODY)
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme::t1())
                                    .child(window_title),
                            ),
                    )
                    // Right: icon-only Dock.
                    .child(
                        div()
                            .id("floating-inspector-redock-btn")
                            .debug_selector(|| "floating-inspector-redock-btn".to_string())
                            .tooltip(theme::tooltip_label("Dock into inspector"))
                            .size(px(24.0))
                            .rounded(theme::RADIUS_MD)
                            .hover(|s| s.bg(theme::bg_3()))
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(render_icon(Icon::ArrowDownToLine, px(14.0), theme::t2()))
                            .on_click(cx.listener(move |_this, _e: &ClickEvent, window, cx| {
                                if let Some(app_entity) = app_redock.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.redock_inspector(cx);
                                    });
                                }
                                window.remove_window();
                            })),
                    ),
            )
            // 2. Revised Inspector Body (fully scrollable, clean layout)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .child(batch.unwrap_or_else(|| {
                        render_inspector_body(InspectorBody {
                            weak_app: self.app.clone(),
                            selected_item,
                            preview_image: viewer_image,
                            tags: &selected_item_tags,
                            suggestions: &suggestions,
                            suggestion_focus,
                            tag_input_active,
                            is_trash,
                            tab,
                            rating,
                            mark_pop: &mark_pop,
                            caption: caption.as_deref(),
                            caption_input_active,
                            title_input_active,
                            date_input_active,
                            location_input_active,
                            field_input,
                            details: info_details.as_ref(),
                        })
                    })),
            )
            .into_any_element()
    }
}
