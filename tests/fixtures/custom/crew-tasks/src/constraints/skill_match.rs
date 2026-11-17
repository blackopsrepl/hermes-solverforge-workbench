use crate::domain::{Plan, Resource, Task};
use solverforge::prelude::*;
use solverforge::stream::joiner::equal_bi;
use solverforge::IncrementalConstraint;

/// HARD: a task goes to a crew member who works its trade.
pub fn constraint() -> impl IncrementalConstraint<Plan, HardSoftScore> {
    ConstraintFactory::<Plan, HardSoftScore>::new()
        .for_each(Plan::tasks())
        .join((Plan::resources(), equal_bi(entity_join_key, fact_join_key)))
        .penalize(hard_weight(join_weight))
        .named("skill_match")
}

fn entity_join_key(entity: &Task) -> Option<usize> {
    entity.resource_idx
}

fn fact_join_key(fact: &Resource) -> Option<usize> {
    Some(fact.slot)
}

fn join_condition(entity: &Task, fact: &Resource) -> bool {
    entity.skill != fact.skill
}

fn join_weight(entity: &Task, fact: &Resource) -> HardSoftScore {
    if join_condition(entity, fact) {
        <HardSoftScore as Score>::one_hard()
    } else {
        <HardSoftScore as Score>::zero()
    }
}
