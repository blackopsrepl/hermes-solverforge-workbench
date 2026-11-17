use crate::domain::{Plan, Task};
use solverforge::prelude::*;
use solverforge::IncrementalConstraint;

/// HARD: every task goes to a crew member.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .filter(unary_condition)
        .penalize(hard_weight(unary_weight))
        .named("assign_every_task")
}

/// Selects each Task this constraint penalizes.
fn unary_condition(entity: &Task) -> bool {
    entity.resource_idx.is_none()
}

/// Penalty for one Task selected by `unary_condition`.
fn unary_weight(_entity: &Task) -> HardSoftScore {
    <HardSoftScore as Score>::one_hard()
}
