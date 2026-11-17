use crate::domain::{Plan, Route};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// SOFT: keep rounds even: each route costs the square of its visit count.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::routes())
        .filter(unary_condition)
        .penalize(unary_weight)
        .named("short_routes")
}

/// Selects each Route that makes a visit.
fn unary_condition(entity: &Route) -> bool {
    !entity.visits.is_empty()
}

/// The square of the route's visit count.
fn unary_weight(entity: &Route) -> HardSoftScore {
    let visits = entity.visits.len() as i64;
    HardSoftScore::of_soft(visits * visits)
}
