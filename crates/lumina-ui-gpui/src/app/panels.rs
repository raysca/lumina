use gpui::*;
use lumina_core::MediaItem;

use crate::app::types::UiMessage;
use crate::app::LuminaApp;
use crate::views::context_menu::ContextMenuState;

impl LuminaApp {
    pub fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        // A failed save is in the banner; put the sidebar back so the
        // screen matches the saved setting.
        if !self.save_config() {
            self.sidebar_collapsed = !self.sidebar_collapsed;
        }
        // The View menu names what the item will do next.
        cx.set_menus(crate::menus::build_menus(self.sidebar_collapsed));
        cx.notify();
    }

    pub fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        // The Albums overview and the Tags page hide the panel: a toggle
        // there would flip and save a setting the user cannot see. So does
        // Review Duplicates.
        if self.non_media_page_shown() || self.duplicates_review_shown() {
            return;
        }
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

    /// ⌘I: the Get Info window on its Info tab. Like Finder, nothing
    /// happens with nothing selected.
    pub fn get_info(&mut self, cx: &mut Context<Self>) {
        if self.selected_item.is_some() {
            self.open_get_info_at(crate::views::inspector::InspectorTab::Info, cx);
        }
    }

    /// Opens the Get Info window on `tab` (⌘I, the context menu and the
    /// palette's Get Info actions).
    pub fn open_get_info_at(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
        self.undock_inspector(cx);
        self.set_inspector_tab(tab, cx);
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

    /// Drops whichever inspector edit is open, unsaved.
    fn cancel_inspector_edits(&mut self, cx: &mut Context<Self>) {
        self.tag_input_active = false;
        self.tag_input_buffer.clear();
        if self.caption.title_input_active {
            self.cancel_title_input(cx);
        }
        if self.caption.date_input_active {
            self.cancel_date_input(cx);
        }
        if self.caption.location_input_active {
            self.cancel_location_input(cx);
        }
        if self.caption.input_active {
            self.cancel_caption_input(cx);
        }
    }

    pub fn set_inspector_tab(
        &mut self,
        tab: crate::views::inspector::InspectorTab,
        cx: &mut Context<Self>,
    ) {
        // The open edit's field may not be on `tab`: end it as Esc would
        // instead of leaving its input taking keys out of sight.
        if tab != self.inspector_tab {
            self.cancel_inspector_edits(cx);
        }
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
        self.context_menu_state = Some(ContextMenuState {
            position,
            item,
            hovered: None,
        });
        // Right-clicking a card can change the selected item; persist it on the
        // same debounced path as keyboard and left-click selection.
        self.schedule_session_save(cx);
        cx.notify();
    }

    pub fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu_state.is_some() {
            self.context_menu_state = None;
            cx.notify();
        }
    }
}

/// What the sidebar toggle does next: "Hide Sidebar" or "Show Sidebar".
pub(crate) fn sidebar_toggle_title(collapsed: bool) -> String {
    crate::i18n::t(if collapsed {
        "sidebar.show"
    } else {
        "sidebar.hide"
    })
}

/// The toolbar and palette icon for the sidebar toggle.
pub(crate) fn sidebar_toggle_icon(collapsed: bool) -> crate::icons::Icon {
    if collapsed {
        crate::icons::Icon::PanelLeftOpen
    } else {
        crate::icons::Icon::PanelLeftClose
    }
}

/// Show/Hide Sidebar, the macOS standard in Finder, Mail and Notes.
pub(crate) const SIDEBAR_SHORTCUT: &str = "⌃⌘S";
