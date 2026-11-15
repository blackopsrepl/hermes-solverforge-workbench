//! Rules over each unit's whole workshop programme.

use solverforge::prelude::*;

use crate::domain::{Plan, Task};
use crate::eval::{double_booked_days, precedence_days, visits, Roster, RosterCollector};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .group_by(
            |task: &Task| task.ctx.task(task.index).unit,
            RosterCollector,
        )
        .penalize(hard_weight(|_: &usize, roster: &Roster| {
            Score::of_hard(double_booked_days(roster))
        }))
        .named("Unit in two bays at once")
        .penalize(hard_weight(|_: &usize, roster: &Roster| {
            Score::of_hard(precedence_days(roster))
        }))
        .named("Task starts before its predecessor finishes")
        .penalize(|_: &usize, roster: &Roster| {
            let extra = (visits(roster) - 1).max(0);
            Score::of_soft(extra * roster.params().map_or(0, |p| p.induction_cost))
        })
        .named("Unit work split over visits")
}
