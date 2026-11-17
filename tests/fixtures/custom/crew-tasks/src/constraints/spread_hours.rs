use crate::domain::{Plan, Task};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// SOFT: spread the hours across the crew: two tasks on one crew member cost
/// the product of their hours.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .filter(has_resource_idx)
        .join(joiner::equal(resource_idx_join_key))
        .filter(pair_condition)
        .penalize(pair_weight)
        .named("spread_hours")
}

/// Unassigned entities share no value, so they never form a pair.
fn has_resource_idx(entity: &Task) -> bool {
    entity.resource_idx.is_some()
}

/// A self-join on this key yields each unordered pair once.
fn resource_idx_join_key(entity: &Task) -> Option<usize> {
    entity.resource_idx
}

/// Selects each pair sharing a resource_idx that this constraint penalizes.
fn pair_condition(_left: &Task, _right: &Task) -> bool {
    true
}

/// Penalty for one pair selected by `pair_condition`.
fn pair_weight(left: &Task, right: &Task) -> HardSoftScore {
    HardSoftScore::of_soft(left.hours * right.hours)
}
