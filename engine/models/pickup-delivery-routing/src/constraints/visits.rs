//! Rules scored per visit, from the route its shift computed or from where
//! it is held.

use solverforge::prelude::*;

use super::rules::*;
use crate::domain::{Plan, Visit};

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

/// `value` while the visit is on a driver shift, else nothing.
fn planned(v: &Visit, value: i64) -> i64 {
    if v.planned() {
        value
    } else {
        0
    }
}

fn flag(on: bool) -> i64 {
    i64::from(on)
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        (
            hard(RIDE_PAST, |v| planned(v, v.loops * HOUR)),
            hard(OVER_CAPACITY, |v| planned(v, v.over_capacity * HOUR)),
            soft(NOT_PREFERRED, |v| planned(v, flag(v.not_preferred))),
            hard(NOT_REQUIRED, |v| planned(v, flag(v.not_required) * HOUR)),
            hard(PROHIBITED_DRIVER, |v| planned(v, flag(v.prohibited) * HOUR)),
            hard(BURDEN, |v| planned(v, v.burden)),
            hard(PROHIBITED_RIDE, |v| planned(v, v.prohibited_ride)),
            hard(POOLING, |v| planned(v, v.pooling)),
            hard(TAGS, |v| planned(v, v.tags_missing * HOUR)),
        ),
        (
            hard(FLOATING_LATE, |v| {
                planned(v, if v.floating_break { v.break_late } else { 0 })
            }),
            hard(FIXED_LATE, |v| {
                planned(
                    v,
                    if v.fixed_break && !v.located {
                        v.break_late
                    } else {
                        0
                    },
                )
            }),
            hard(APPOINTMENT_LATE, |v| {
                planned(
                    v,
                    if v.fixed_break && v.located {
                        v.break_late
                    } else {
                        0
                    },
                )
            }),
            hard(FIXED_DURING_JOB, |v| {
                planned(v, if v.fixed_break { v.break_during_job } else { 0 })
            }),
            hard(FLOATING_DURING_JOB, |v| {
                planned(
                    v,
                    if v.floating_break {
                        v.break_during_job
                    } else {
                        0
                    },
                )
            }),
            soft(EARLIEST_DAY, |v| planned(v, v.earliest_day)),
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
                Some(shift) if v.shift != Some(shift) || v.benched => PIN_SHIFT_COST,
                _ => 0,
            }),
            hard(PIN_START, |v| planned(v, v.pin_moved)),
            soft(WAITING, |v| planned(v, v.wait)),
            hard(UNREACHABLE_STOP, |v| planned(v, flag(v.unreachable) * HOUR)),
            soft(SLA_MISSED, |v| match v.status() {
                crate::domain::Status::Planned => v.sla_late,
                _ => v.sla_unplanned,
            }),
            hard(SKILLS_MISSING, |v| planned(v, v.skills_missing * HOUR)),
            soft(OVERQUALIFIED, |v| planned(v, v.overqualified)),
            hard(WINDOW_END, |v| planned(v, v.late_end)),
            hard(WINDOW_START, |v| planned(v, v.late_start)),
        ),
    )
}
