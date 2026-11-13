//! Constraints over each employee's whole roster. They share one grouped
//! node: a move re-scores only the employees whose shifts changed.

use solverforge::prelude::*;

use crate::derived::Measure;
use crate::domain::{Employee, Plan, Shift};
use crate::eval::{
    day_runs, near_day_off, rules, shift_runs, window_penalty, Roster, RosterCollector,
};

type Score = HardMediumSoftScore;

/// Hard weight from a per-employee amount.
macro_rules! hard {
    ($amount:expr) => {
        hard_weight(move |e: &usize, r: &Roster| Score::of_hard($amount(r, *e)))
    };
}

/// Soft weight from a per-employee amount.
macro_rules! soft {
    ($amount:expr) => {
        move |e: &usize, r: &Roster| Score::of_soft($amount(r, *e))
    };
}

fn window(measure: Measure, rolling: bool, preferred: bool) -> impl Fn(&Roster, usize) -> i64 {
    move |r, e| window_penalty(r, e, rules(measure, rolling, preferred))
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    use Measure as M;
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .group_by(
            |shift: &Shift| shift.employee_idx.unwrap_or(usize::MAX),
            RosterCollector,
        )
        .complement_with_key(
            Plan::employees(),
            |shift: &Shift| shift.employee_idx,
            |employee: &Employee| employee.index,
            |employee: &Employee| Roster::empty(employee.ctx.clone()),
        )
        // Work limits.
        .penalize(hard!(window(M::Minutes, false, false)))
        .named("Minutes per period outside required range")
        .penalize(soft!(window(M::Minutes, false, true)))
        .named("Minutes per period outside preferred range")
        .penalize(hard!(window(M::Minutes, true, false)))
        .named("Minutes in rolling window outside required range")
        .penalize(soft!(window(M::Minutes, true, true)))
        .named("Minutes in rolling window outside preferred range")
        .penalize(hard!(window(M::Logged, false, false)))
        .named("Logged minutes per period outside required range")
        .penalize(soft!(window(M::Logged, false, true)))
        .named("Logged minutes per period outside preferred range")
        .penalize(hard!(window(M::Days, false, false)))
        .named("Days worked per period outside required range")
        .penalize(soft!(window(M::Days, false, true)))
        .named("Days worked per period outside preferred range")
        .penalize(hard!(window(M::Days, true, false)))
        .named("Days worked in rolling window outside required range")
        .penalize(soft!(window(M::Days, true, true)))
        .named("Days worked in rolling window outside preferred range")
        .penalize(hard!(window(M::Shifts, false, false)))
        .named("Employee shifts per period outside required range")
        .penalize(soft!(window(M::Shifts, false, true)))
        .named("Employee shifts per period outside preferred range")
        .penalize(hard!(window(M::Shifts, true, false)))
        .named("Employee shifts in rolling window outside required range")
        .penalize(soft!(window(M::Shifts, true, true)))
        .named("Employee shifts in rolling window outside preferred range")
        .penalize(hard!(window(M::Weekends, false, false)))
        .named("Weekends worked per period outside required range")
        .penalize(soft!(window(M::Weekends, false, true)))
        .named("Weekends worked per period outside preferred range")
        .penalize(hard!(window(M::Weekends, true, false)))
        .named("Weekends worked in rolling window outside required range")
        .penalize(soft!(window(M::Weekends, true, true)))
        .named("Weekends worked in rolling window outside preferred range")
        .penalize(hard!(window(M::WeekendMinutes, false, false)))
        .named("Weekend minutes per period outside required range")
        .penalize(soft!(window(M::WeekendMinutes, false, true)))
        .named("Weekend minutes per period outside preferred range")
        .penalize(hard!(window(M::ConsecutiveWeekends, false, false)))
        .named("Consecutive weekends worked outside required range")
        .penalize(soft!(window(M::ConsecutiveWeekends, false, true)))
        .named("Consecutive weekends worked outside preferred range")
        .penalize(hard!(|r: &Roster, e| day_runs(r, e, false).length))
        .named("Consecutive days worked outside required range")
        .penalize(soft!(|r: &Roster, e| day_runs(r, e, true).length))
        .named("Consecutive days worked outside preferred range")
        .penalize(hard!(|r: &Roster, e| shift_runs(r, e, false).0))
        .named("Consecutive shifts outside required range")
        .penalize(soft!(|r: &Roster, e| shift_runs(r, e, true).0))
        .named("Consecutive shifts outside preferred range")
        .penalize(hard!(|r: &Roster, e| shift_runs(r, e, false).1))
        .named("Rest after a run of shifts below required minimum")
        .penalize(soft!(|r: &Roster, e| shift_runs(r, e, true).1))
        .named("Rest after a run of shifts below preferred minimum")
        .penalize(hard!(|r: &Roster, e| day_runs(r, e, false).gap))
        .named("Gap between differing runs below required minimum")
        .penalize(soft!(|r: &Roster, e| day_runs(r, e, true).gap))
        .named("Gap between differing runs below preferred minimum")
        .penalize(hard!(|r: &Roster, e| day_runs(r, e, false).bad_start))
        .named("Run starts on a disallowed day")
        .reward(soft!(|r: &Roster, e| day_runs(r, e, true).good_start))
        .named("Run starts on an allowed day")
        .penalize(soft!(|r: &Roster, e| {
            day_runs(r, e, false).ends_on_start + day_runs(r, e, true).ends_on_start
        }))
        .named("Run ends on a start day")
        .reward(soft!(|r: &Roster, e| {
            day_runs(r, e, false).compact + day_runs(r, e, true).compact
        }))
        .named("Compact run of one category")
        // Time off.
        .penalize(hard!(window(M::DaysOff, false, false)))
        .named("Days off per period outside required range")
        .penalize(soft!(window(M::DaysOff, false, true)))
        .named("Days off per period outside preferred range")
        .penalize(hard!(window(M::ConsecutiveDaysOff, false, false)))
        .named("Consecutive days off per period below required minimum")
        .penalize(soft!(window(M::ConsecutiveDaysOff, false, true)))
        .named("Consecutive days off per period below preferred minimum")
        .penalize(hard!(window(M::ConsecutiveDaysOff, true, false)))
        .named("Consecutive days off in rolling window below required minimum")
        .penalize(soft!(window(M::ConsecutiveDaysOff, true, true)))
        .named("Consecutive days off in rolling window below preferred minimum")
        .penalize(hard!(window(M::MinutesOff, true, false)))
        .named("Continuous rest in rolling window below required minimum")
        .penalize(soft!(window(M::MinutesOff, true, true)))
        .named("Continuous rest in rolling window below preferred minimum")
        .penalize(hard!(window(M::ConsecutiveWeekendsOff, false, false)))
        .named("Consecutive weekends off per period below required minimum")
        .penalize(soft!(window(M::ConsecutiveWeekendsOff, false, true)))
        .named("Consecutive weekends off per period below preferred minimum")
        .penalize(hard!(|r: &Roster, e| near_day_off(r, e, true)))
        .named("Barred shift next to a requested day off")
        .penalize(soft!(|r: &Roster, e| near_day_off(r, e, false)))
        .named("Unwanted shift next to a requested day off")
}
