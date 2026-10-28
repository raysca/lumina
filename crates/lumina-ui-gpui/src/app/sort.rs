//! Grid sorting controls.

use gpui::*;
use lumina_core::{SortDirection, SortField, SortSpec};

use crate::app::LuminaApp;

impl LuminaApp {
    pub fn sort_field_label(&self) -> &'static str {
        match self.sort.field {
            SortField::DateTaken => "Date",
            SortField::FileName => "Name",
            SortField::FileSize => "Size",
            SortField::Rating => "Rating",
        }
    }

    pub fn sort_label(&self) -> String {
        format!(
            "{} {}",
            self.sort_field_label(),
            match self.sort.direction {
                SortDirection::Ascending => "↑",
                SortDirection::Descending => "↓",
            }
        )
    }

    pub fn toggle_sort_menu(&mut self, cx: &mut Context<Self>) {
        self.sort_menu_open = !self.sort_menu_open;
        cx.notify();
    }

    pub fn close_sort_menu(&mut self, cx: &mut Context<Self>) {
        if self.sort_menu_open {
            self.sort_menu_open = false;
            cx.notify();
        }
    }

    pub fn set_sort(&mut self, field: SortField, direction: SortDirection, cx: &mut Context<Self>) {
        self.sort = SortSpec { field, direction };
        self.sort_menu_open = false;
        self.save_config();
        self.clear_selection(cx);
        self.set_last_action(format!("Sorted by {}", self.sort_label()));
        cx.notify();
    }

    pub fn set_sort_field(&mut self, field: SortField, cx: &mut Context<Self>) {
        self.set_sort(field, self.sort.direction, cx);
    }

    pub fn set_sort_direction(&mut self, direction: SortDirection, cx: &mut Context<Self>) {
        self.set_sort(self.sort.field, direction, cx);
    }
}
