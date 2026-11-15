//! Small shapes and serde defaults shared by the records.

use serde::{Deserialize, Serialize};

/// An inclusive range of whole days, `YYYY-MM-DD` to `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Span {
    pub from: String,
    /// The last day of the range, included.
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Span {
    pub fn new(from: impl Into<String>, to: impl Into<String>, label: &str) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            label: Some(label.to_string()),
        }
    }
}

pub fn one() -> i64 {
    1
}

pub fn is_one(value: &i64) -> bool {
    *value == 1
}

pub fn is_zero(value: &i64) -> bool {
    *value == 0
}
