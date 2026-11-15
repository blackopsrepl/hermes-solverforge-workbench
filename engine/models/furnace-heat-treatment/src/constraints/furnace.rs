//! Constraints on where and when loads run: furnace fit, capacity,
//! overlaps, downtime, the horizon, release and energy.

use solverforge::prelude::*;
use solverforge::stream::collector::{count, sum};
use solverforge::stream::joiner::equal;

use crate::domain::{Plan, WorkOrder};
use crate::time::overlap;

type Score = HardMediumSoftScore;

fn f() -> ConstraintFactory<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
}

/// °C a scheduled order's setpoint exceeds its furnace's maximum.
fn too_hot(order: &WorkOrder) -> i64 {
    order.placed().map_or(0, |p| {
        let max = order.ctx.furnaces[p.furnace].max_temperature;
        (order.ctx.recipe_of(order.index).temperature - max).max(0)
    })
}

fn wrong_process(order: &WorkOrder) -> bool {
    order.placed().is_some_and(|p| {
        order.ctx.furnaces[p.furnace].processes & order.ctx.recipe_of(order.index).process.bit()
            == 0
    })
}

fn past_horizon(order: &WorkOrder) -> i64 {
    order
        .placed()
        .map_or(0, |p| (p.end - order.ctx.horizon).max(0))
}

fn downtime_minutes(order: &WorkOrder) -> i64 {
    order.placed().map_or(0, |p| {
        order.ctx.furnaces[p.furnace]
            .downtime
            .iter()
            .map(|&(from, to)| overlap(p.start, p.end, from, to))
            .sum()
    })
}

fn early_minutes(order: &WorkOrder) -> i64 {
    match (order.placed(), order.ctx.orders[order.index].release) {
        (Some(p), Some(release)) => (release - p.start).max(0),
        _ => 0,
    }
}

/// Key of the load a scheduled order belongs to: `(slot value, recipe)`.
fn load_key(order: &WorkOrder) -> (usize, usize) {
    (order.slot.unwrap_or(usize::MAX), order.recipe_idx())
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| o.slot.is_none())
            .penalize(|o: &WorkOrder| Score::of(0, o.ctx.orders[o.index].priority.weight(), 0))
            .named("Work order not scheduled"),
        f().for_each(Plan::work_orders())
            .filter(wrong_process)
            .penalize(hard_weight(|_: &WorkOrder| Score::of_hard(4_000)))
            .named("Furnace cannot run the process"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| too_hot(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| Score::of_hard(too_hot(o) * 20)))
            .named("Setpoint above furnace maximum"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| o.slot.is_some())
            .group_by(
                |o: &WorkOrder| {
                    let (slot, recipe) = load_key(o);
                    let furnace = slot / o.ctx.slots;
                    (slot, recipe, o.ctx.furnaces[furnace].max_load_kg)
                },
                sum(|o: &WorkOrder| o.load_kg),
            )
            .penalize(hard_weight(|key: &(usize, usize, i64), kg: &i64| {
                Score::of_hard((*kg - key.2).max(0) * 10)
            }))
            .named("Furnace load over capacity"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| past_horizon(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(past_horizon(o) * 5)
            }))
            .named("Load runs past the schedule"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| o.slot.is_some())
            .join(equal(|o: &WorkOrder| o.furnace_idx()))
            .filter(|a: &WorkOrder, b: &WorkOrder| !a.same_load(b) && load_overlap(a, b) > 0)
            .penalize(hard_weight(|a: &WorkOrder, b: &WorkOrder| {
                Score::of_hard(load_overlap(a, b) * 20)
            }))
            .named("Loads overlap in a furnace"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| downtime_minutes(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(downtime_minutes(o) * 20)
            }))
            .named("Load during furnace downtime"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| early_minutes(o) > 0)
            .penalize(hard_weight(|o: &WorkOrder| {
                Score::of_hard(early_minutes(o) * 10)
            }))
            .named("Load starts before parts arrive"),
        f().for_each(Plan::work_orders())
            .filter(|o: &WorkOrder| o.slot.is_some())
            .group_by(
                |o: &WorkOrder| {
                    let p = o.placed().expect("filtered to scheduled orders");
                    let (slot, recipe) = load_key(o);
                    (slot, recipe, o.ctx.energy_cents(p.furnace, p.start, p.end))
                },
                count(),
            )
            .penalize(|key: &(usize, usize, i64), _: &usize| Score::of_soft(key.2))
            .named("Energy cost"),
    )
}

/// Minutes two scheduled orders overlap in time.
pub fn load_overlap(a: &WorkOrder, b: &WorkOrder) -> i64 {
    match (a.placed(), b.placed()) {
        (Some(x), Some(y)) => overlap(x.start, x.end, y.start, y.end),
        _ => 0,
    }
}
