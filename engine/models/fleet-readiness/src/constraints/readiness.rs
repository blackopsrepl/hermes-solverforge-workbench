//! Fleet readiness day by day: every day of the horizon is scored, days the
//! plan keeps no unit down included.

use solverforge::prelude::*;

use crate::domain::{Day, Plan};
use crate::eval::{downtime_cost, readiness_gap, ByDown, Floor, Row, UnitSet, UnitSetCollector};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .project(ByDown)
        .group_by(|row: &Row| row.bucket as usize, UnitSetCollector)
        .complement(
            Plan::days(),
            |day: &Day| day.index,
            |day: &Day| UnitSet::empty(day.ctx.clone()),
        )
        .penalize(hard_weight(|day: &usize, down: &UnitSet| {
            Score::of_hard(readiness_gap(*day, down, Floor::Minimum))
        }))
        .named("Readiness below minimum")
        .penalize(|day: &usize, down: &UnitSet| {
            let gap = readiness_gap(*day, down, Floor::Target);
            Score::of_soft(gap * down.derived().map_or(0, |d| d.params.target_shortfall_cost))
        })
        .named("Readiness below target")
        .penalize(|day: &usize, down: &UnitSet| Score::of_soft(downtime_cost(*day, down)))
        .named("Unit downtime")
}
