//! Constraints over each employee's whole roster. They share one grouped
//! node: a move re-scores only the employees whose shifts changed.

use solverforge::prelude::*;

use crate::derived::{Measure, Want};
use crate::domain::{Employee, Plan, Shift};
use crate::eval::{
    activation, day_patterns, day_runs, disruption, fill_up, gaps, location_change_shortfall,
    multi_day_patterns, near_day_off, overlap_minutes, pairings, rotation_blocks, rules,
    shift_runs, unfairness, weekly_rotation, window_penalty, Roster, RosterCollector,
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

/// One grouped node over every employee's roster, employees without shifts
/// included. Each node carries at most a few dozen terminals: the compiler
/// nests one type level per terminal.
macro_rules! rosters {
    () => {
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
    };
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        work_limits(),
        time_off_and_patterns(),
        variety_and_more(),
        economy(),
    )
}

fn economy() -> impl ConstraintSet<Plan, Score> {
    use Measure as M;
    rosters!()
        .penalize(hard!(window(M::Cost, false, false)))
        .named("Contract cost outside required range")
        .penalize(soft!(window(M::Cost, false, true)))
        .named("Contract cost")
        .penalize(soft!(activation))
        .named("Employee activated")
        .reward(soft!(fill_up))
        .named("Activated employee filled up")
        .penalize(soft!(unfairness))
        .named("Unfair workload")
        .penalize(hard!(|r: &Roster, e| disruption(r, e, false)))
        .named("Published shift reassigned against rule")
        .penalize(soft!(|r: &Roster, e| disruption(r, e, true)))
        .named("Published shift reassigned")
}

fn work_limits() -> impl ConstraintSet<Plan, Score> {
    use Measure as M;
    rosters!()
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
}

fn time_off_and_patterns() -> impl ConstraintSet<Plan, Score> {
    use Measure as M;
    rosters!()
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
        // Rotations and patterns.
        .penalize(hard!(|r: &Roster, e| overlap_minutes(r, e)))
        .named("Overlapping shifts")
        .penalize(hard!(|r: &Roster, e| gaps(r, e, false, false)))
        .named("Rest between shifts outside required range")
        .penalize(soft!(|r: &Roster, e| gaps(r, e, false, true)))
        .named("Rest between shifts outside preferred range")
        .penalize(hard!(window(M::StartSpread, false, false)))
        .named("Start time spread per period outside required range")
        .penalize(soft!(window(M::StartSpread, false, true)))
        .named("Start time spread per period outside preferred range")
        .reward(soft!(
            |r: &Roster, e| gaps(r, e, true, false) + gaps(r, e, true, true)
        ))
        .named("Split shifts close together")
        .penalize(hard!(|r: &Roster, e| pairings(r, e, Want::Required)))
        .named("Required shift pairing missing")
        .penalize(soft!(|r: &Roster, e| pairings(r, e, Want::Preferred)))
        .named("Preferred shift pairing missing")
        .penalize(hard!(|r: &Roster, e| pairings(r, e, Want::Barred)))
        .named("Barred shift pairing worked")
        .penalize(soft!(|r: &Roster, e| pairings(r, e, Want::Unwanted)))
        .named("Unwanted shift pairing worked")
        .penalize(hard!(|r: &Roster, e| day_patterns(r, e, Want::Required)))
        .named("Required day pattern broken")
        .reward(soft!(|r: &Roster, e| day_patterns(r, e, Want::Preferred)))
        .named("Preferred day pattern worked")
        .penalize(hard!(|r: &Roster, e| day_patterns(r, e, Want::Barred)))
        .named("Barred day pattern worked")
        .penalize(soft!(|r: &Roster, e| day_patterns(r, e, Want::Unwanted)))
        .named("Unwanted day pattern worked")
        .penalize(hard!(|r: &Roster, e| multi_day_patterns(
            r,
            e,
            Want::Required
        )))
        .named("Required multi-day pattern broken")
        .reward(soft!(|r: &Roster, e| multi_day_patterns(
            r,
            e,
            Want::Preferred
        )))
        .named("Preferred multi-day pattern worked")
        .penalize(hard!(|r: &Roster, e| multi_day_patterns(
            r,
            e,
            Want::Barred
        )))
        .named("Barred multi-day pattern worked")
        .penalize(soft!(|r: &Roster, e| multi_day_patterns(
            r,
            e,
            Want::Unwanted
        )))
        .named("Unwanted multi-day pattern worked")
        .penalize(hard!(|r: &Roster, e| weekly_rotation(r, e, false)))
        .named("Required weekly rotation broken")
        .penalize(soft!(|r: &Roster, e| weekly_rotation(r, e, true)))
        .named("Preferred weekly rotation broken")
        .penalize(hard!(|r: &Roster, e| rotation_blocks(r, e, false)))
        .named("Rotation block longer than required maximum")
        .penalize(soft!(|r: &Roster, e| rotation_blocks(r, e, true)))
        .named("Rotation block length outside preferred range")
}

fn variety_and_more() -> impl ConstraintSet<Plan, Score> {
    use Measure as M;
    rosters!()
        .penalize(hard!(window(M::ShiftTypes, false, false)))
        .named("Shift types per period outside required range")
        .penalize(soft!(window(M::ShiftTypes, false, true)))
        .named("Shift types per period outside preferred range")
        .penalize(hard!(window(M::DistinctTags, false, false)))
        .named("Distinct tags per period for employee outside required range")
        .penalize(soft!(window(M::DistinctTags, false, true)))
        .named("Distinct tags per period for employee outside preferred range")
        .penalize(hard!(|r: &Roster, e| location_change_shortfall(r, e)))
        .named("Too little time to change location")
        .penalize(hard!(window(M::Locations, false, false)))
        .named("Locations per period outside required range")
        .penalize(soft!(window(M::Locations, false, true)))
        .named("Locations per period outside preferred range")
}
