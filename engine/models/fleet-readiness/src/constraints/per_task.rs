//! Rules about one task and the slot it takes: its window, due limits, bay
//! and cost, whether it is planned at all, and whether it moved.

use solverforge::prelude::*;

use crate::derived::Derived;
use crate::domain::{Plan, Task};

type Score = HardMediumSoftScore;

/// A fact about task `t` in slot `s`.
type SlotFact = fn(&Derived, usize, usize) -> i64;

fn amount(task: &Task, fact: SlotFact) -> i64 {
    task.slot_idx
        .map_or(0, |slot| fact(&task.ctx, task.index, slot))
}

/// Hard penalty of `fact` per planned task.
fn hard(name: &str, fact: SlotFact) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .filter(move |task: &Task| amount(task, fact) != 0)
        .penalize(hard_weight(move |task: &Task| {
            Score::of_hard(amount(task, fact))
        }))
        .named(name)
}

fn unplanned(task: &Task, deferrable: bool) -> bool {
    task.slot_idx.is_none() && task.ctx.task(task.index).deferrable == deferrable
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    let f = ConstraintFactory::<Plan, Score>::new;
    (
        f().for_each(Plan::tasks())
            .filter(|task: &Task| unplanned(task, false))
            .penalize(|task: &Task| Score::of(0, task.ctx.priority(task.index), 0))
            .named("Mandatory task unplanned"),
        f().for_each(Plan::tasks())
            .filter(|task: &Task| unplanned(task, true))
            .penalize(|task: &Task| {
                Score::of_soft(task.ctx.params.deferral_cost * task.ctx.priority(task.index))
            })
            .named("Deferrable task unplanned"),
        hard("Task starts before its earliest start", Derived::early_days),
        hard("Task finishes after its due date", Derived::late_days),
        hard("Task starts past its usage limit", Derived::usage_days),
        hard("Bay unsuitable for task", Derived::unsuitable_days),
        hard("Task in a closed bay", Derived::closed_days),
        hard("Maintenance while unit is away", Derived::away_days),
        f().for_each(Plan::tasks())
            .filter(|task: &Task| amount(task, Derived::bay_cost) != 0)
            .penalize(|task: &Task| Score::of_soft(amount(task, Derived::bay_cost)))
            .named("Bay cost"),
        f().for_each(Plan::tasks())
            .filter(|task: &Task| task.ctx.moved(task.index, task.slot_idx))
            .penalize(|task: &Task| Score::of_soft(task.ctx.params.change_cost))
            .named("Published task moved"),
    )
}
