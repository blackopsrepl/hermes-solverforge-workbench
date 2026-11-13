//! Which demos exist and how each is built.

use crate::domain::Plan;

/// Demo ids, default first.
pub fn ids() -> Vec<&'static str> {
    vec!["SMALL_WEEK"]
}

/// A prepared demo plan.
pub fn build(id: &str) -> Option<Plan> {
    let mut plan = match id {
        "SMALL_WEEK" => super::small::small_week(),
        _ => return None,
    };
    crate::prepare::prepare(&mut plan);
    Some(plan)
}
