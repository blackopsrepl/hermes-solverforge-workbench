//! Rules scored per driver shift, from its route totals, on the shift's
//! last visit: its slot holds the drive so far and the drive back to the
//! end location. A shift with no stop drives nowhere and costs nothing.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Visit};
use crate::route::Slot;

pub type Score = HardMediumSoftScore;

type Amount = fn(&Slot) -> i64;

/// What the route ending at `v` costs, if `v` ends a driver shift's route.
fn last(v: &Visit, amount: Amount) -> i64 {
    match v.planned_slot() {
        Some(slot) if v.route_next.is_none() && slot.state.active => amount(slot),
        _ => 0,
    }
}

fn hard(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(move |v: &Visit| last(v, amount) != 0)
        .penalize(hard_weight(move |v: &Visit| {
            Score::of_hard(last(v, amount))
        }))
        .named(name)
}

fn soft(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(move |v: &Visit| last(v, amount) != 0)
        .penalize(move |v: &Visit| Score::of_soft(last(v, amount)))
        .named(name)
}

/// A route's driver and its working seconds, start to end, on its last visit.
fn work(v: &Visit) -> Option<(usize, i64)> {
    match v.planned_slot() {
        Some(slot) if v.route_next.is_none() && slot.state.active => Some((
            slot.state.driver,
            (slot.end.arrival - slot.state.start).max(0),
        )),
        _ => None,
    }
}

fn drivers_used() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(|v: &Visit| work(v).is_some())
        .group_by(|v: &Visit| work(v).map_or(usize::MAX, |w| w.0), count())
        .penalize(|_: &usize, _: &usize| Score::of_soft(HOUR))
        .named(DRIVER_USED)
}

fn balance() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(|v: &Visit| work(v).is_some())
        .group_by(
            |v: &Visit| work(v).map_or(usize::MAX, |w| w.0),
            sum(|v: &Visit| work(v).map_or(0, |w| w.1)),
        )
        .penalize(|_: &usize, seconds: &i64| {
            let minutes = seconds / 60;
            Score::of_soft(minutes * minutes)
        })
        .named(DRIVER_BALANCE)
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        drivers_used(),
        balance(),
        hard(UNREACHABLE_ROUTE, |s| {
            (s.state.unreachable_legs + i64::from(s.end.unreachable)) * HOUR
        }),
        soft(TRAVEL, |s| s.state.travel + s.end.travel),
        hard(LAST_LATE, |s| s.end.last_late),
        hard(END_LATE, |s| s.end.late),
        soft(LAST_SOFT_LATE, |s| s.end.last_soft_late),
        soft(END_SOFT_LATE, |s| s.end.soft_late),
    )
}
