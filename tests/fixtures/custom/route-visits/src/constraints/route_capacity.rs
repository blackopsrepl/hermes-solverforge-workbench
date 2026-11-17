use crate::domain::{Plan, Route};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// HARD: a route makes no more visits than its van's capacity.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::routes())
        .filter(unary_condition)
        .penalize(hard_weight(unary_weight))
        .named("route_capacity")
}

/// Selects each Route over its capacity.
fn unary_condition(entity: &Route) -> bool {
    entity.visits.len() > entity.capacity
}

/// One hard point per visit over capacity.
fn unary_weight(entity: &Route) -> HardSoftScore {
    HardSoftScore::of_hard((entity.visits.len() - entity.capacity) as i64)
}
