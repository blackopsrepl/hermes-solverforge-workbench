//! Due dates and start times.

use solverforge::prelude::*;

use crate::domain::{Plan, WorkOrder};
use crate::wire::{Priority, ShiftKind};

type Score = HardMediumSoftScore;

fn late(order: &WorkOrder) -> i64 {
    order
        .placed()
        .map_or(0, |p| (p.end - order.ctx.orders[order.index].due).max(0))
}

fn early(order: &WorkOrder) -> i64 {
    order
        .placed()
        .map_or(0, |p| (order.ctx.orders[order.index].due - p.end).max(0))
}

fn priority(order: &WorkOrder) -> Priority {
    order.ctx.orders[order.index].priority
}

fn lateness(name: &'static str, band: Priority) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::work_orders())
        .filter(move |o: &WorkOrder| priority(o) == band && late(o) > 0)
        .penalize(move |o: &WorkOrder| Score::of_soft(late(o) * band.lateness_per_minute()))
        .named(name)
}

/// Whether a scheduled order of a day process is charged on a night shift.
pub fn night_start(order: &WorkOrder) -> bool {
    order.placed().is_some_and(|p| {
        !order.ctx.recipe_of(order.index).process.runs_overnight()
            && order
                .ctx
                .owner(p.start)
                .is_some_and(|shift| shift.kind == ShiftKind::Night)
    })
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    let f = ConstraintFactory::<Plan, Score>::new;
    (
        lateness("Express order late", Priority::Express),
        lateness("Urgent order late", Priority::Urgent),
        lateness("Standard order late", Priority::Standard),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| early(o) >= 40)
            .penalize(|o: &WorkOrder| Score::of_soft(early(o) / 40))
            .named("Finished long before due"),
        f().for_each(Plan::work_orders())
            .filter(night_start)
            .penalize(|o: &WorkOrder| Score::of_soft(priority(o).night_start_cost()))
            .named("Day process charged at night"),
    )
}
