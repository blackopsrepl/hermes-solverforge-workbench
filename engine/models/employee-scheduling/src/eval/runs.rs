//! Runs of consecutive days or shifts, and shifts next to requested days off.

use super::Roster;
use crate::derived::{outside, Derived, ShiftInfo};
use crate::time::DAY;

/// Weighted scores of an employee's consecutive-days rules of one level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DayRunScores {
    /// Run lengths outside `[min, max]`.
    pub length: i64,
    /// Minutes missing between runs of different categories.
    pub gap: i64,
    /// Runs starting on a weekday that is not an allowed start day.
    pub bad_start: i64,
    /// Runs starting on an allowed start day.
    pub good_start: i64,
    /// Runs whose last day is an allowed start day.
    pub ends_on_start: i64,
    /// Days continuing the previous day's category.
    pub compact: i64,
}

struct Run<'a> {
    first: i64,
    last: i64,
    category: u32,
    first_shift: &'a ShiftInfo,
    last_shift: &'a ShiftInfo,
}

fn touches_horizon(d: &Derived, first_day: i64, last_day: i64) -> bool {
    first_day * DAY < d.horizon_end && (last_day + 1) * DAY > d.horizon_start
}

/// Scores the consecutive-days rules of `preferred` level for employee `e`.
pub fn day_runs(roster: &Roster, e: usize, preferred: bool) -> DayRunScores {
    let mut out = DayRunScores::default();
    let Some(d) = roster.derived() else {
        return out;
    };
    for rule in d.employees[e]
        .day_runs
        .iter()
        .filter(|r| r.preferred == preferred)
    {
        let mut runs: Vec<Run> = Vec::new();
        for (_, info) in roster.iter(d) {
            if !rule.shifts.accepts(&info.tags) {
                continue;
            }
            let category = if rule.categories.is_empty() {
                0
            } else {
                match rule
                    .categories
                    .iter()
                    .find(|c| info.tags.binary_search(c).is_ok())
                {
                    Some(&c) => c,
                    None => continue,
                }
            };
            match runs.last_mut() {
                Some(run) if run.last == info.day => {
                    if info.end > run.last_shift.end {
                        run.last_shift = info;
                    }
                }
                Some(run) if run.last + 1 == info.day && run.category == category => {
                    run.last = info.day;
                    run.last_shift = info;
                }
                _ => runs.push(Run {
                    first: info.day,
                    last: info.day,
                    category,
                    first_shift: info,
                    last_shift: info,
                }),
            }
        }
        let mut s = DayRunScores::default();
        for (n, run) in runs.iter().enumerate() {
            if !touches_horizon(d, run.first, run.last) {
                continue;
            }
            s.length += outside(run.last - run.first + 1, rule.min, rule.max);
            if rule.start_days != 0 {
                if rule.start_days & (1 << d.weekday(run.first)) != 0 {
                    s.good_start += 1;
                } else {
                    s.bad_start += 1;
                }
                if rule.start_days & (1 << d.weekday(run.last)) != 0 {
                    s.ends_on_start += 1;
                }
            }
            if rule.compact {
                s.compact += run.last - run.first;
            }
            if let (Some(min_gap), Some(prev)) = (rule.min_gap, n.checked_sub(1).map(|p| &runs[p]))
            {
                if prev.category != run.category {
                    let gap = run.first_shift.start - prev.last_shift.end;
                    s.gap += (min_gap - gap).max(0);
                }
            }
        }
        out.length += s.length * rule.weight;
        out.gap += s.gap * rule.weight;
        out.bad_start += s.bad_start * rule.weight;
        out.good_start += s.good_start * rule.weight;
        out.ends_on_start += s.ends_on_start * rule.weight;
        out.compact += s.compact * rule.weight;
    }
    out
}

/// Weighted scores of an employee's consecutive-shifts rules of one level:
/// `(run lengths outside [min, max], minutes of rest missing after runs)`.
pub fn shift_runs(roster: &Roster, e: usize, preferred: bool) -> (i64, i64) {
    let Some(d) = roster.derived() else {
        return (0, 0);
    };
    let (mut length, mut rest) = (0, 0);
    for rule in d.employees[e]
        .shift_runs
        .iter()
        .filter(|r| r.preferred == preferred)
    {
        let items: Vec<&ShiftInfo> = roster
            .iter(d)
            .map(|(_, info)| info)
            .filter(|info| rule.shifts.accepts(&info.tags))
            .collect();
        // Runs as [from, to) ranges of `items`.
        let mut runs: Vec<(usize, usize, i64)> = Vec::new();
        for (n, info) in items.iter().enumerate() {
            match runs.last_mut() {
                Some((_, to, end)) if info.start - *end <= rule.link => {
                    *to = n + 1;
                    *end = (*end).max(info.end);
                }
                _ => runs.push((n, n + 1, info.end)),
            }
        }
        let (mut l, mut r) = (0, 0);
        for (k, &(from, to, end)) in runs.iter().enumerate() {
            if !touches_horizon(d, items[from].day, items[to - 1].last_day) {
                continue;
            }
            l += outside((to - from) as i64, rule.min, rule.max);
            if let (Some(need), Some(next)) = (rule.rest_after, runs.get(k + 1)) {
                if (to - from) as i64 >= rule.rest_after_runs_of {
                    r += (need - (items[next.0].start - end)).max(0);
                }
            }
        }
        length += l * rule.weight;
        rest += r * rule.weight;
    }
    (length, rest)
}

/// Weighted count of shifts next to a requested day off, for `barred` or
/// unwanted rules.
pub fn near_day_off(roster: &Roster, e: usize, barred: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let employee = &d.employees[e];
    if employee.requested_off.is_empty() {
        return 0;
    }
    let mut total = 0;
    for rule in employee.near_day_off.iter().filter(|r| r.barred == barred) {
        let mut hits = 0;
        for (_, info) in roster.iter(d) {
            let requested = |day: i64| employee.requested_off.binary_search(&day).is_ok();
            if rule.before.as_ref().is_some_and(|f| f.accepts(&info.tags))
                && requested(info.day + 1)
            {
                hits += 1;
            }
            if rule.after.as_ref().is_some_and(|f| f.accepts(&info.tags)) && requested(info.day - 1)
            {
                hits += 1;
            }
        }
        total += hits * rule.weight;
    }
    total
}
