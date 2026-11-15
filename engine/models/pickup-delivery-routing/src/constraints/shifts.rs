//! Rules scored per driver shift, from its route totals.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Shift};

pub type Score = HardMediumSoftScore;

type Amount = fn(&Shift) -> i64;

fn hard(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(move |s: &Shift| !s.bench && amount(s) != 0)
        .penalize(hard_weight(move |s: &Shift| Score::of_hard(amount(s))))
        .named(name)
}

fn soft(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(move |s: &Shift| !s.bench && amount(s) != 0)
        .penalize(move |s: &Shift| Score::of_soft(amount(s)))
        .named(name)
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        hard(UNREACHABLE_ROUTE, |s| s.unreachable_legs * HOUR),
        soft(TRAVEL, |s| s.travel),
        hard(LAST_LATE, |s| s.last_late),
        hard(END_LATE, |s| s.end_late),
        soft(LAST_SOFT_LATE, |s| s.last_soft_late),
        soft(END_SOFT_LATE, |s| s.end_soft_late),
    )
}
