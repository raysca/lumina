//! Keyboard focus in the builder: the focusable controls and the Tab order.

use super::{DraftRow, ValueKind};

/// A keyboard-focusable control in the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftFocus {
    Title,
    Match,
    Field(usize),
    Operator(usize),
    Value(usize),
    Unit(usize),
    Remove(usize),
    Add(usize),
    Pin,
    Cancel,
    Save,
}

impl DraftFocus {
    /// The condition row this control belongs to, if any.
    pub fn row(self) -> Option<usize> {
        match self {
            Self::Field(i)
            | Self::Operator(i)
            | Self::Value(i)
            | Self::Unit(i)
            | Self::Remove(i)
            | Self::Add(i) => Some(i),
            _ => None,
        }
    }
}

/// An open option popup and its highlighted option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftPopup {
    pub target: DraftFocus,
    pub highlight: usize,
}

/// Tab order for the current rows.
pub fn focus_order(rows: &[DraftRow]) -> Vec<DraftFocus> {
    let mut order = vec![DraftFocus::Title, DraftFocus::Match];
    for (i, row) in rows.iter().enumerate() {
        order.push(DraftFocus::Field(i));
        order.push(DraftFocus::Operator(i));
        if row.kind() != ValueKind::None {
            order.push(DraftFocus::Value(i));
        }
        if row.kind() == ValueKind::Period {
            order.push(DraftFocus::Unit(i));
        }
        order.push(DraftFocus::Remove(i));
        order.push(DraftFocus::Add(i));
    }
    // The empty state's "Add Condition" button.
    if rows.is_empty() {
        order.push(DraftFocus::Add(0));
    }
    order.extend([DraftFocus::Pin, DraftFocus::Cancel, DraftFocus::Save]);
    order
}
