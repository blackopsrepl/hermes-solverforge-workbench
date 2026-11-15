//! Which demos exist and how each is built.

use crate::domain::Plan;

/// Demo ids, default first.
pub fn ids() -> Vec<&'static str> {
    vec!["SMALL_FORTNIGHT", "LARGE_SIX_WEEKS"]
}

/// A prepared demo plan.
pub fn build(id: &str) -> Option<Plan> {
    let mut plan = match id {
        "SMALL_FORTNIGHT" => super::small::small_fortnight(),
        "LARGE_SIX_WEEKS" => super::large::large_six_weeks(),
        _ => return None,
    };
    crate::prepare::prepare(&mut plan);
    Some(plan)
}
