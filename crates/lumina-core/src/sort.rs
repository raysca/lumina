//! User-selectable library ordering.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortField {
    #[default]
    DateTaken,
    FileName,
    FileSize,
    Rating,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortDirection {
    #[default]
    Descending,
    Ascending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SortSpec {
    pub field: SortField,
    pub direction: SortDirection,
}

impl SortSpec {
    pub fn is_default(&self) -> bool {
        *self == SortSpec::default()
    }

    pub fn field_key(self) -> &'static str {
        match self.field {
            SortField::DateTaken => "date",
            SortField::FileName => "name",
            SortField::FileSize => "size",
            SortField::Rating => "rating",
        }
    }

    pub fn direction_key(self) -> &'static str {
        match self.direction {
            SortDirection::Ascending => "asc",
            SortDirection::Descending => "desc",
        }
    }

    /// SQL ORDER BY fragment for the given table alias/prefix (e.g. "m.").
    pub fn order_by(&self, prefix: &str) -> String {
        let column = match self.field {
            SortField::DateTaken => format!("{prefix}timestamp_epoch_ms"),
            SortField::FileName => format!("{prefix}file_name COLLATE NOCASE"),
            SortField::FileSize => format!("{prefix}file_size"),
            SortField::Rating => format!("{prefix}rating"),
        };
        let direction = match self.direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        format!("{column} {direction}, {prefix}id ASC")
    }
}
