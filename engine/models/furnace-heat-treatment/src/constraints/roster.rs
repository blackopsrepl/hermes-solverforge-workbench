//! The operators' roster: coverage, nights, rest, weekly limits, leave and fairness.

use solverforge::prelude::*;
use solverforge::stream::collector::{count, sum};
use solverforge::stream::joiner::equal;

use crate::domain::{Plan, RosterDay};
use crate::eval::{Demand, NightWindows, Staffing, Supply};
use crate::wire::{Role, ShiftKind};

type Score = HardMediumSoftScore;

fn f() -> ConstraintFactory<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
}

/// Minutes of rest missing between two roster days of one operator, in
/// either order.
pub fn rest_shortage(a: &RosterDay, b: &RosterDay) -> i64 {
    let (first, second) = if a.day_index() <= b.day_index() {
        (a, b)
    } else {
        (b, a)
    };
    if second.day_index() != first.day_index() + 1 {
        return 0;
    }
    let (Some(x), Some(y)) = (first.shift_id(), second.shift_id()) else {
        return 0;
    };
    let d = &a.ctx;
    let rest = d.shift_bounds(y).0 - d.shift_bounds(x).1;
    (d.s.minimum_rest - rest).max(0)
}

/// Whether a roster row is the unavailable operator working.
pub fn works_while_unavailable(row: &RosterDay) -> bool {
    row.is_working()
        && row.ctx.operators[row.operator_idx()]
            .unavailable
            .contains(&row.day_index())
}

/// `(operator, week, most shifts, target shifts)` of a worked day.
fn week(row: &RosterDay) -> (usize, i64, i64, i64) {
    let s = &row.ctx.s;
    (
        row.operator_idx(),
        row.day_index() / 7,
        s.max_shifts_per_week,
        s.target_shifts_per_week,
    )
}

/// Shifts per operator and week, against the most allowed and the target.
fn weekly() -> impl ConstraintSet<Plan, Score> {
    f().for_each(Plan::roster())
        .filter(RosterDay::works_in_schedule)
        .group_by(week, count())
        .penalize(hard_weight(|w: &(usize, i64, i64, i64), shifts: &usize| {
            Score::of_hard((*shifts as i64 - w.2).max(0) * 900)
        }))
        .named("Too many shifts in a week")
        .penalize(|w: &(usize, i64, i64, i64), shifts: &usize| {
            Score::of_soft((*shifts as i64 - w.3).max(0) * 240)
        })
        .named("Shifts above the weekly target")
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        f().for_each(Plan::coverage())
            .project(Demand)
            .merge(f().for_each(Plan::roster()).project(Supply))
            .group_by(
                |s: &Staffing| (s.shift, s.role),
                sum(|s: &Staffing| s.delta),
            )
            .penalize(hard_weight(|_: &(usize, Role), missing: &i64| {
                Score::of_hard((*missing).max(0) * 900)
            }))
            .named("Shift short of a role"),
        f().for_each(Plan::roster())
            .filter(|r: &RosterDay| {
                r.kind() == Some(ShiftKind::Night) && r.ctx.operators[r.operator_idx()].day_only
            })
            .penalize(hard_weight(|_: &RosterDay| Score::of_hard(1_200)))
            .named("Day-only operator on nights"),
        f().for_each(Plan::roster())
            .filter(|r: &RosterDay| r.is_working())
            .join(equal(|r: &RosterDay| r.operator_idx()))
            .filter(|a: &RosterDay, b: &RosterDay| rest_shortage(a, b) > 0)
            .penalize(hard_weight(|a: &RosterDay, b: &RosterDay| {
                Score::of_hard(rest_shortage(a, b))
            }))
            .named("Rest between shifts too short"),
        weekly(),
        f().for_each(Plan::roster())
            .project(NightWindows)
            .group_by(|w: &(usize, i64, i64)| *w, count())
            .penalize(hard_weight(|w: &(usize, i64, i64), nights: &usize| {
                Score::of_hard((*nights as i64 - w.2).max(0) * 900)
            }))
            .named("Too many nights in a row"),
        f().for_each(Plan::roster())
            .filter(works_while_unavailable)
            .penalize(hard_weight(|_: &RosterDay| Score::of_hard(1_200)))
            .named("Operator rostered while unavailable"),
        f().for_each(Plan::roster())
            .filter(RosterDay::works_in_schedule)
            .balance(|r: &RosterDay| Some(r.operator_idx()))
            .penalize(Score::of_soft(120))
            .named("Uneven shifts across operators"),
    )
}
