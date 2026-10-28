use gpui::*;
use lumina_core::MediaItem;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::context_menu::ContextMenuState;

impl LuminaApp {
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        self.save_config();
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        if self.floating_inspector_window.is_some() {
            // If already floating, focus the floating window
            self.inspector_open = false;
            self.save_config();
            cx.notify();
            return;
        }
        self.inspector_open = !self.inspector_open;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    pub fn undock_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = false;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();

        if self.floating_inspector_window.is_some() {
            cx.notify();
            return;
        }

        // Defer window creation to next tick of GPUI event loop
        let _ = self.ui_tx.send_blocking(UiMessage::OpenFloatingInspector);
        cx.notify();
    }

    pub fn redock_inspector(&mut self, cx: &mut Context<Self>) {
        self.floating_inspector_window = None;
        self.inspector_open = true;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    pub fn floating_inspector_closed(&mut self, cx: &mut Context<Self>) {
        self.floating_inspector_window = None;
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        self.save_config();
        cx.notify();
    }

    pub fn set_inspector_tab(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
        self.inspector_tab = tab;
        cx.notify();
    }

    pub fn open_card_context_menu(
        &mut self,
        item: MediaItem,
        index: usize,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if !self.selected_indices.contains(&index) {
            self.selected_indices.clear();
            self.selected_indices.insert(index);
            self.selection_anchor = Some(index);
            self.selected_index = Some(index);
            self.select_item_internal(item.clone(), cx);
        }
        self.context_menu_state = Some(ContextMenuState { position, item });
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu_state.is_some() {
            self.context_menu_state = None;
            cx.notify();
        }
    }

    pub fn zoom_in_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_columns > 2 {
            self.grid_columns -= 1;
            self.save_config();
            cx.notify();
        }
    }

    pub fn zoom_out_grid(&mut self, cx: &mut Context<Self>) {
        if self.grid_columns < 8 {
            self.grid_columns += 1;
            self.save_config();
            cx.notify();
        }
    }

    pub fn start_caption_input(&mut self, cx: &mut Context<Self>) {
        let existing = self
            .selected_item
            .as_ref()
            .and_then(|it| self.item_captions.get(&it.id))
            .cloned()
            .unwrap_or_default();
        self.caption.input_active = true;
        self.caption.input_buffer = existing;
        cx.notify();
    }

    pub fn commit_caption_input(&mut self, cx: &mut Context<Self>) {
        let caption = std::mem::take(&mut self.caption.input_buffer);
        self.caption.input_active = false;
        if let Some(ref item) = self.selected_item {
            let trimmed = caption.trim().to_string();
            if trimmed.is_empty() {
                self.item_captions.remove(&item.id);
            } else {
                self.item_captions.insert(item.id, trimmed.clone());
            }
            if let Some(ref state) = self.grid_state {
                let _ = state.engine.set_caption(item.id, &trimmed);
            }
        }
        cx.notify();
    }

    pub fn cancel_caption_input(&mut self, cx: &mut Context<Self>) {
        self.caption.input_active = false;
        self.caption.input_buffer.clear();
        cx.notify();
    }
}
