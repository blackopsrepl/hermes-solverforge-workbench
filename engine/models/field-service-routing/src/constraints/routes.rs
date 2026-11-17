//! Rules scored per shift, on its last element: the slot there holds the
//! drive so far and the drive back to the end location.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Visit};

pub type Score = HardMediumSoftScore;

/// What the route ending at `v` costs under `rule`, if `v` ends a route.
fn last(v: &Visit, rule: Rule) -> i64 {
    match v.planned_slot() {
        Some(slot) if v.route_next.is_none() => slot.end.amount(rule),
        _ => 0,
    }
}

fn per_route(rule: Rule) -> impl IncrementalConstraint<Plan, Score> {
    let level = rule.level();
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(move |v: &Visit| last(v, rule) != 0)
        .penalize(move |v: &Visit| {
            let amount = last(v, rule);
            match level {
                solverforge_workbench_host::Level::Hard => Score::of_hard(amount),
                solverforge_workbench_host::Level::Medium => Score::of_medium(amount),
                solverforge_workbench_host::Level::Soft => Score::of_soft(amount),
            }
        })
        .named(rule.name())
}

macro_rules! per_route {
    ($($rule:ident),* $(,)?) => {
        ($(per_route(Rule::$rule)),*)
    };
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        per_route!(
            MovableBalance,
            MaxServiceHard,
            MaxServiceSoft,
            MaxTravelHard,
            MaxTravelSoft,
            MaxVisitsHard,
            MaxVisitsSoft,
            TravelDistance,
            TravelTime,
        ),
        per_route!(
            InactiveDistance,
            InactiveTime,
            ReachableItinerary,
            Overnight,
            LastDepartureHard,
            LastDepartureSoft,
            EndHard,
            EndSoft,
            ShiftCost,
        ),
    )
}
