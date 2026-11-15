//! The demo catalog.

use crate::domain::Plan;

/// Demo ids.
pub fn ids() -> Vec<&'static str> {
    vec!["MILAN_TWO_DAYS"]
}

/// Builds one demo dataset.
pub fn build(id: &str) -> Option<Plan> {
    match id {
        "MILAN_TWO_DAYS" => Some(super::milan::milan_two_days()),
        _ => None,
    }
}
