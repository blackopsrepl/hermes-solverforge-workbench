//! Rules scored per element, from the amounts its slot carries; and the
//! visits that cannot be timed.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Status, Visit};

pub type Score = HardMediumSoftScore;

/// The rule's amount on a visit: from its slot, or what it costs unplanned
/// while it is in no list.
fn amount(v: &Visit, rule: Rule) -> i64 {
    match (&v.slot, v.shift) {
        (Some(slot), _) => slot.amount(rule),
        (None, None) => v
            .missing
            .iter()
            .find(|(r, _)| *r == rule)
            .map_or(0, |(_, amount)| *amount),
        (None, Some(_)) => 0,
    }
}

fn score(rule: Rule, amount: i64) -> Score {
    match rule.level() {
        solverforge_workbench_host::Level::Hard => Score::of_hard(amount),
        solverforge_workbench_host::Level::Medium => Score::of_medium(amount),
        solverforge_workbench_host::Level::Soft => Score::of_soft(amount),
    }
}

/// One per-element rule, read from the slot; a rewarding rule's amount
/// counts negative, so it adds to the score.
fn per_visit(rule: Rule) -> impl IncrementalConstraint<Plan, Score> {
    let sign = if rule.rewards() { -1 } else { 1 };
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(move |v: &Visit| amount(v, rule) != 0)
        .penalize(move |v: &Visit| score(rule, sign * amount(v, rule)))
        .named(rule.name())
}

/// A visit on a shift that cannot be timed: it waits, directly or through
/// its group, on a visit after it.
fn loops() -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::elements())
        .filter(|v: &Visit| v.status() == Status::Untimed)
        .penalize(hard_weight(|v: &Visit| {
            Score::of_hard(v.service + BROKEN_LINK)
        }))
        .named(Rule::Loops.name())
}

macro_rules! per_visit {
    ($($rule:ident),* $(,)?) => {
        ($(per_visit(Rule::$rule)),*)
    };
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        per_visit!(
            DependencyDelay,
            DependencySameVehicle,
            DependencySameShift,
            DependencyPrerequisite,
            DependencyPooling,
            FixedBreakDelayed,
            FloatingBreakLate,
            FixedBreakConflict,
            LocatedBreakConflict,
            EarliestDay,
        ),
        per_visit!(
            CompletionRisk,
            OptionalUnplanned,
            MandatoryUnplanned,
            Profit,
            PinStart,
            PinShift,
            PinStartDelay,
            Disruption,
            MaxTravelPerVisit,
            Waiting,
        ),
        per_visit!(
            Colocated,
            Consecutive,
            TowardsEnd,
            ReachableVisit,
            SlaEnd,
            OptionalSla,
            SlaEarlier,
            MandatorySla,
            UnnecessarySkills,
            SkillsMissing,
        ),
        per_visit!(
            OutsidePreferredArea,
            OutsideArea,
            Rating,
            PreferredWindow,
            WindowEnd,
            WindowStart,
            PreferredVehicle,
            TagsMissing,
            NotRequiredVehicle,
            ProhibitedVehicle,
        ),
        loops(),
    )
}
