//! Constraints scored per assigned shift from static (shift, employee) facts.

use solverforge::prelude::*;

use crate::derived::Derived;
use crate::domain::{Plan, Shift};

pub type Score = HardMediumSoftScore;

/// A static fact about shift `s` worked by employee `e`.
pub type PairFact = fn(&Derived, usize, usize) -> i64;

fn amount(shift: &Shift, fact: PairFact) -> i64 {
    match shift.employee_idx {
        Some(employee) => fact(&shift.ctx, shift.index, employee),
        None => 0,
    }
}

fn priority(shift: &Shift) -> i64 {
    shift
        .employee_idx
        .map_or(1, |employee| shift.ctx.employee(employee).priority)
}

/// Hard penalty of `fact` per assigned shift.
pub fn hard(name: &str, fact: PairFact) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(move |shift: &Shift| amount(shift, fact) != 0)
        .penalize(hard_weight(move |shift: &Shift| {
            Score::of_hard(amount(shift, fact))
        }))
        .named(name)
}

/// Soft penalty of `fact` per assigned shift, times the employee's priority.
pub fn soft(name: &str, fact: PairFact) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(move |shift: &Shift| amount(shift, fact) != 0)
        .penalize(move |shift: &Shift| Score::of_soft(amount(shift, fact) * priority(shift)))
        .named(name)
}

/// Soft reward of `fact` per assigned shift, times the employee's priority.
pub fn bonus(name: &str, fact: PairFact) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(move |shift: &Shift| amount(shift, fact) != 0)
        .reward(move |shift: &Shift| Score::of_soft(amount(shift, fact) * priority(shift)))
        .named(name)
}
