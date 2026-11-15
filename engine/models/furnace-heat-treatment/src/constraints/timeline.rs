//! Changeovers between consecutive loads of each furnace. Both rules share
//! one grouped node: a move re-scores only the furnaces it touches.

use solverforge::prelude::*;

use crate::domain::{Plan, WorkOrder};
use crate::eval::{Timeline, TimelineCollector};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::work_orders())
        .filter(|o: &WorkOrder| o.slot.is_some())
        .group_by(
            |o: &WorkOrder| o.furnace_idx().unwrap_or(usize::MAX),
            TimelineCollector,
        )
        .penalize(hard_weight(|_: &usize, t: &Timeline| {
            Score::of_hard(t.shortage() * 10)
        }))
        .named("Changeover time too short")
        .penalize(|_: &usize, t: &Timeline| Score::of_soft(t.changeover_minutes()))
        .named("Changeover time")
}
