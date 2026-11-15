//! Bays and crews day by day: units per bay and technicians per crew.

use solverforge::prelude::*;

use crate::domain::Plan;
use crate::eval::{
    bay_excess, crew_load, ByBayDay, ByCrewDay, Load, LoadCollector, Row, UnitSet, UnitSetCollector,
};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (bays(), crews())
}

fn bays() -> impl ConstraintSet<Plan, Score> {
    (ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .project(ByBayDay)
        .group_by(|row: &Row| row.bucket as usize, UnitSetCollector)
        .penalize(hard_weight(|bucket: &usize, set: &UnitSet| {
            Score::of_hard(bay_excess(*bucket, set))
        }))
        .named("Bay over capacity"),)
}

fn crews() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .project(ByCrewDay)
        .group_by(|row: &Row| row.bucket as usize, LoadCollector)
        .penalize(hard_weight(|bucket: &usize, load: &Load| {
            Score::of_hard(crew_load(*bucket, load).beyond)
        }))
        .named("Crew over capacity")
        .penalize(|bucket: &usize, load: &Load| {
            let overtime = crew_load(*bucket, load).overtime;
            Score::of_soft(overtime * load_cost(load, |p| p.overtime_cost))
        })
        .named("Overtime")
        .penalize(|bucket: &usize, load: &Load| {
            let above = crew_load(*bucket, load).above_level;
            Score::of_soft(above * load_cost(load, |p| p.load_level_cost))
        })
        .named("Uneven crew load")
}

fn load_cost(load: &Load, pick: fn(&crate::wire::Parameters) -> i64) -> i64 {
    load.params().map_or(0, pick)
}
