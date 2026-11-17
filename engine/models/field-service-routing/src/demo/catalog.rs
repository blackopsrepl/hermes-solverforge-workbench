//! The demo catalog.

use crate::domain::Plan;

/// Demo ids.
pub fn ids() -> Vec<&'static str> {
    vec!["TURIN_TWO_DAYS"]
}

/// Builds one demo dataset.
pub fn build(id: &str) -> Option<Plan> {
    match id {
        "TURIN_TWO_DAYS" => Some(super::turin::turin_two_days()),
        _ => None,
    }
}
