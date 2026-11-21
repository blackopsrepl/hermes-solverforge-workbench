//! Rules over several rows together: visits that exclude each other, a
//! planned visit whose dependent is not, multi-vehicle groups planned in
//! part, linked shifts used with their components, shared tools, and
//! fairness between technicians.

use solverforge::prelude::*;
use solverforge::stream::joiner::equal_bi;

use super::rules::*;
use crate::domain::{Plan, Shift, Status, Visit};
use crate::tools::{waiting, Used};

pub type Score = HardMediumSoftScore;

fn exclusions() -> impl IncrementalConstraint<Plan, Score> {
    let excluding = || {
        ConstraintFactory::<Plan, Score>::new()
            .for_each(Plan::elements())
            .filter(|v: &Visit| !v.exclusions.is_empty() && v.planned())
    };
    excluding()
        .join((
            excluding(),
            equal_bi(|a: &Visit| a.exclusion_set, |b: &Visit| b.exclusion_set),
        ))
        .filter(|a: &Visit, b: &Visit| a.index < b.index && a.exclusions.contains(&b.index))
        .penalize(hard_weight(|_: &Visit, _: &Visit| Score::of_hard(HOUR)))
        .named(Rule::Exclusion.name())
}

/// A planned visit some unplanned visit depends on.
fn dependent_unplanned() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(|d: &Visit| !d.preceding.is_empty() && d.status() == Status::Benched)
        .join((
            ConstraintFactory::<Plan, Score>::new()
                .for_each(Plan::elements())
                .filter(|p: &Visit| p.has_dependents && p.planned()),
            equal_bi(|d: &Visit| d.dependency_set, |p: &Visit| p.dependency_set),
        ))
        .filter(|d: &Visit, p: &Visit| d.preceding.contains(&p.index))
        .penalize(hard_weight(|d: &Visit, _: &Visit| {
            Score::of_hard(d.service + BROKEN_LINK)
        }))
        .named(Rule::DependentUnplanned.name())
}

/// One placed member of a group: whether it is planned, and its service
/// seconds as given.
type Member = (bool, i64);

fn semi_assigned(members: &CollectedVec<Member>) -> i64 {
    if !members.iter().any(|m| m.0) {
        return 0;
    }
    members
        .iter()
        .filter(|m| !m.0)
        .map(|m| m.1 + BROKEN_LINK)
        .sum()
}

fn groups() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(|v: &Visit| {
            v.group.is_some() && matches!(v.status(), Status::Planned | Status::Benched)
        })
        .group_by(
            |v: &Visit| v.group.unwrap_or(usize::MAX),
            collect_vec(|v: &Visit| (v.planned(), v.service)),
        )
        .penalize(hard_weight(
            |_group: &usize, members: &CollectedVec<Member>| Score::of_hard(semi_assigned(members)),
        ))
        .named(Rule::SemiAssigned.name())
}

/// A linked shift serving visits while one of its components does too,
/// costing per visit on the component, so emptying the component one visit
/// at a time pays off.
fn excludes_shifts() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(|l: &Shift| !l.components.is_empty() && l.serves())
        .join((
            ConstraintFactory::<Plan, Score>::new()
                .for_each(Plan::shifts())
                .filter(|c: &Shift| c.components.is_empty() && c.serves()),
            equal_bi(|l: &Shift| l.vehicle, |c: &Shift| c.vehicle),
        ))
        .filter(|l: &Shift, c: &Shift| l.components.contains(&c.index))
        .penalize(hard_weight(|_: &Shift, c: &Shift| {
            Score::of_hard(c.served() as i64 * HOUR)
        }))
        .named(Rule::ExcludesShifts.name())
}

/// Seconds a vehicle worked and had available before this plan, and the
/// seconds it has available in this plan.
pub type History = (i64, i64, i64);

/// A route's work, keyed by its vehicle and the vehicle's history, on the
/// route's last element.
fn work(v: &Visit) -> Option<((usize, History), i64)> {
    match v.planned_slot() {
        Some(slot) if v.route_next.is_none() && slot.state.active => {
            Some(((slot.vehicle, slot.end.history), slot.end.work))
        }
        _ => None,
    }
}

fn fairness() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(|v: &Visit| work(v).is_some())
        .group_by(
            |v: &Visit| work(v).map_or((usize::MAX, (0, 0, 0)), |(key, _)| key),
            sum(|v: &Visit| work(v).map_or(0, |(_, w)| w)),
        )
        .penalize(|key: &(usize, History), worked: &i64| {
            let (_, (utilized, capacity, available)) = *key;
            let total = (capacity + available).max(1);
            let share = (utilized + worked) * 10_000 / total;
            Score::of_soft(share * share / 10_000)
        })
        .named(Rule::Fairness.name())
}

/// Visits that need shared tools, replayed tool by tool in start order.
fn tools() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(|v: &Visit| v.tools.is_some() && v.planned())
        .group_by(
            |_: &Visit| 0usize,
            collect_vec(|v: &Visit| -> Used {
                let at = v.at().copied().unwrap_or_default();
                (at.start, at.end, v.tools.clone().expect("needs tools"))
            }),
        )
        .penalize(hard_weight(|_: &usize, used: &CollectedVec<Used>| {
            Score::of_hard(waiting(used.iter()))
        }))
        .named(Rule::ToolBusy.name())
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        exclusions(),
        tools(),
        dependent_unplanned(),
        groups(),
        excludes_shifts(),
        fairness(),
    )
}
