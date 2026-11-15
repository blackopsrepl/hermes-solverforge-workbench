//! Which demos exist and how each is built.

use crate::domain::Plan;

/// Demo ids.
pub fn ids() -> Vec<&'static str> {
    vec!["LARGE_MONTH"]
}

/// A prepared demo plan.
pub fn build(id: &str) -> Option<Plan> {
    let mut plan = match id {
        "LARGE_MONTH" => super::large::large_month(),
        _ => return None,
    };
    crate::prepare::prepare(&mut plan);
    Some(plan)
}
