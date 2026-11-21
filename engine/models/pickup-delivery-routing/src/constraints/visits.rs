//! Rules scored per visit, from its slot or from where it is held.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Visit};
use crate::route::At;

pub type Score = HardMediumSoftScore;

/// What a visit costs under one rule.
type Amount = fn(&Visit) -> i64;

fn hard(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(move |v: &Visit| amount(v) != 0)
        .penalize(hard_weight(move |v: &Visit| Score::of_hard(amount(v))))
        .named(name)
}

fn medium(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(move |v: &Visit| amount(v) != 0)
        .penalize(move |v: &Visit| Score::of_medium(amount(v)))
        .named(name)
}

fn soft(name: &'static str, amount: Amount) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::visits())
        .filter(move |v: &Visit| amount(v) != 0)
        .penalize(move |v: &Visit| Score::of_soft(amount(v)))
        .named(name)
}

/// What a visit planned as `at` costs; nothing while it is not planned.
fn planned(v: &Visit, amount: impl Fn(&At) -> i64) -> i64 {
    v.at().map_or(0, amount)
}

fn flag(on: bool) -> i64 {
    i64::from(on)
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        (
            hard(RIDE_PAST, |v| planned(v, |at| at.loops * HOUR)),
            hard(OVER_CAPACITY, |v| planned(v, |at| at.over_capacity * HOUR)),
            soft(NOT_PREFERRED, |v| planned(v, |at| flag(at.not_preferred))),
            hard(NOT_REQUIRED, |v| {
                planned(v, |at| flag(at.not_required) * HOUR)
            }),
            hard(PROHIBITED_DRIVER, |v| {
                planned(v, |at| flag(at.prohibited) * HOUR)
            }),
            hard(BURDEN, |v| planned(v, |at| at.burden)),
            hard(PROHIBITED_RIDE, |v| planned(v, |at| at.prohibited_ride)),
            hard(POOLING, |v| planned(v, |at| at.pooling)),
            hard(TAGS, |v| planned(v, |at| at.tags_missing * HOUR)),
            hard(PARTS, |v| planned(v, |at| at.parts_missing * HOUR)),
        ),
        (
            hard(FLOATING_LATE, |v| {
                planned(v, |at| if v.floating_break { at.break_late } else { 0 })
            }),
            hard(FIXED_LATE, |v| {
                planned(v, |at| {
                    if v.fixed_break && !v.located {
                        at.break_late
                    } else {
                        0
                    }
                })
            }),
            hard(APPOINTMENT_LATE, |v| {
                planned(v, |at| {
                    if v.fixed_break && v.located {
                        at.break_late
                    } else {
                        0
                    }
                })
            }),
            hard(FIXED_DURING_JOB, |v| {
                planned(v, |at| {
                    if v.fixed_break {
                        at.break_during_job
                    } else {
                        0
                    }
                })
            }),
            hard(FLOATING_DURING_JOB, |v| {
                planned(v, |at| {
                    if v.floating_break {
                        at.break_during_job
                    } else {
                        0
                    }
                })
            }),
            soft(EARLIEST_DAY, |v| planned(v, |at| at.earliest_day)),
            medium(MANDATORY, |v| {
                if v.unplanned() {
                    v.unplanned_medium
                } else {
                    0
                }
            }),
            soft(
                OPTIONAL,
                |v| if v.unplanned() { v.unplanned_soft } else { 0 },
            ),
        ),
        (
            hard(PIN_SHIFT, |v| match v.pin_shift {
                Some(shift) if v.shift != Some(shift) || !v.planned() => PIN_SHIFT_COST,
                _ => 0,
            }),
            hard(PIN_START, |v| planned(v, |at| at.pin_moved)),
            soft(WAITING, |v| planned(v, |at| at.wait)),
            hard(UNREACHABLE_STOP, |v| {
                planned(v, |at| at.unreachable_legs * HOUR)
            }),
            soft(SLA_MISSED, |v| match v.status() {
                crate::domain::Status::Planned => planned(v, |at| at.sla_late),
                _ => v.sla_unplanned,
            }),
            hard(SKILLS_MISSING, |v| {
                planned(v, |at| at.skills_missing * HOUR)
            }),
            soft(OVERQUALIFIED, |v| planned(v, |at| at.overqualified)),
            hard(WINDOW_END, |v| planned(v, |at| at.late_end)),
            hard(WINDOW_START, |v| planned(v, |at| at.late_start)),
        ),
    )
}
