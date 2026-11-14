//! Window rules: a measure of an employee's shifts per period or rolling window.
//!
//! These run for every changed employee on every move, once per window rule
//! terminal, so they reuse per-thread buffers instead of allocating.

use std::cell::RefCell;

use super::Roster;
use crate::derived::{outside, Derived, Measure, ShiftInfo, WindowRule};
use crate::time::DAY;

thread_local! {
    /// Shift indexes a rule counts, sorted by start.
    static COUNTED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// Values to count once each.
    static DISTINCT: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

/// Weighted deviation of every window rule matching `select`, for employee `e`.
pub fn window_penalty(roster: &Roster, e: usize, select: impl Fn(&WindowRule) -> bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let mut total = 0;
    for rule in d.employees[e].window_rules.iter().filter(|r| select(r)) {
        let deviation = if rule.shifts.is_everything() {
            deviation(d, e, rule, roster.indexes())
        } else {
            COUNTED.with_borrow_mut(|counted| {
                counted.clear();
                counted.extend(
                    roster
                        .indexes()
                        .iter()
                        .copied()
                        .filter(|&i| rule.shifts.accepts(&d.shifts[i as usize].tags)),
                );
                deviation(d, e, rule, counted)
            })
        };
        total += deviation * rule.weight;
    }
    total
}

fn deviation(d: &Derived, e: usize, rule: &WindowRule, items: &[u32]) -> i64 {
    rule.windows
        .iter()
        .map(|&window| window_deviation(d, e, rule, items, window))
        .sum()
}

/// Selects rules by measure, window type and level.
pub fn rules(measure: Measure, rolling: bool, preferred: bool) -> impl Fn(&WindowRule) -> bool {
    move |rule| rule.measure == measure && rule.rolling == rolling && rule.preferred == preferred
}

/// The shifts of `items` that start in the window, and end in it for
/// whole-shift rules.
fn members<'a>(
    d: &'a Derived,
    rule: &'a WindowRule,
    items: &'a [u32],
    (start, end): (i64, i64),
) -> impl Iterator<Item = &'a ShiftInfo> + 'a {
    let start_of = |i: &u32| d.shifts[*i as usize].start;
    let lo = items.partition_point(|i| start_of(i) < start);
    let hi = items.partition_point(|i| start_of(i) < end);
    items[lo..hi]
        .iter()
        .map(|&i| &d.shifts[i as usize])
        .filter(move |info| !rule.whole_shift || info.end <= end)
}

/// Runs `f` on the distinct values, sorted.
fn with_distinct<T>(values: impl Iterator<Item = i64>, f: impl FnOnce(&[i64]) -> T) -> T {
    DISTINCT.with_borrow_mut(|out| {
        out.clear();
        out.extend(values);
        out.sort_unstable();
        out.dedup();
        f(out)
    })
}

fn distinct_count(values: impl Iterator<Item = i64>) -> i64 {
    with_distinct(values, |v| v.len() as i64)
}

/// Plan days of `window`, limited to the data.
fn days_of(d: &Derived, (start, end): (i64, i64)) -> std::ops::Range<i64> {
    start.div_euclid(DAY).max(0)..((end - 1).div_euclid(DAY) + 1).min(d.days)
}

fn longest_run(flags: impl Iterator<Item = bool>) -> i64 {
    let (mut best, mut current) = (0, 0);
    for flag in flags {
        current = if flag { current + 1 } else { 0 };
        best = best.max(current);
    }
    best
}

fn window_deviation(
    d: &Derived,
    e: usize,
    rule: &WindowRule,
    items: &[u32],
    window: (i64, i64),
) -> i64 {
    let inside = || members(d, rule, items, window);
    let (min, max) = (rule.min, rule.max);
    let weekends = || {
        inside()
            .filter(|i| d.is_weekend(i.day))
            .map(|i| d.weekend_of(i.day))
    };
    match rule.measure {
        Measure::Minutes => outside(inside().map(|i| i.minutes).sum(), min, max),
        Measure::Logged => {
            let before: i64 = d.employees[e]
                .logged_before
                .iter()
                .filter(|(day, _)| (window.0..window.1).contains(&(day * DAY)))
                .map(|(_, minutes)| minutes)
                .sum();
            outside(before + inside().map(|i| i.logged).sum::<i64>(), min, max)
        }
        Measure::Days => outside(distinct_count(inside().map(|i| i.day)), min, max),
        Measure::Shifts => outside(inside().count() as i64, min, max),
        Measure::Cost => {
            let tiers = rule.cost.as_ref().expect("cost rules carry tiers");
            let cents = tiers.cents(inside().map(|i| i.minutes).sum());
            if min.is_none() && max.is_none() {
                cents
            } else {
                outside(cents, min, max)
            }
        }
        Measure::Weekends => outside(distinct_count(weekends()), min, max),
        Measure::WeekendMinutes => outside(inside().map(|i| i.weekend_minutes).sum(), min, max),
        Measure::ConsecutiveWeekends => with_distinct(weekends(), |worked| {
            let mut penalty = 0;
            let mut run = 0;
            for (n, &key) in worked.iter().enumerate() {
                run = if n > 0 && key - worked[n - 1] == 7 {
                    run + 1
                } else {
                    if run > 0 {
                        penalty += outside(run, min, max);
                    }
                    1
                };
            }
            if run > 0 {
                penalty += outside(run, min, max);
            }
            penalty
        }),
        Measure::DaysOff => with_distinct(inside().map(|i| i.day), |worked| {
            let off = days_of(d, window)
                .filter(|&day| rule.weekdays & (1 << d.weekday(day)) != 0)
                .filter(|day| worked.binary_search(day).is_err())
                .count() as i64;
            outside(off, min, max)
        }),
        Measure::ConsecutiveDaysOff => with_distinct(inside().map(|i| i.day), |worked| {
            let longest =
                longest_run(days_of(d, window).map(|day| worked.binary_search(&day).is_err()));
            outside(longest, min, None)
        }),
        Measure::MinutesOff => {
            let (start, end) = window;
            let mut cursor = start;
            let mut longest = 0;
            for info in items
                .iter()
                .map(|&i| &d.shifts[i as usize])
                .filter(|i| i.start < end && i.end > start)
            {
                longest = longest.max(info.start - cursor);
                cursor = cursor.max(info.end);
            }
            longest = longest.max(end - cursor);
            outside(longest, min, None)
        }
        Measure::ShiftTypes => rule
            .tags
            .iter()
            .map(|category| {
                let n = inside()
                    .filter(|i| i.tags.binary_search(category).is_ok())
                    .count() as i64;
                outside(n, min, max)
            })
            .sum(),
        Measure::DistinctTags => {
            let used = distinct_count(
                inside()
                    .flat_map(|i| i.tags.iter())
                    .filter(|tag| rule.tags.binary_search(tag).is_ok())
                    .map(|&tag| i64::from(tag)),
            );
            outside(used, min, max)
        }
        Measure::Locations => outside(
            distinct_count(inside().filter_map(|i| i.location_id.map(i64::from))),
            min,
            max,
        ),
        Measure::StartSpread => {
            let (lo, hi) = inside()
                .map(|i| i.start.rem_euclid(DAY))
                .fold((i64::MAX, i64::MIN), |(lo, hi), t| (lo.min(t), hi.max(t)));
            if lo <= hi {
                outside(hi - lo, min, max)
            } else {
                0
            }
        }
        Measure::ConsecutiveWeekendsOff => with_distinct(weekends(), |worked| {
            let weekends = days_of(d, window).filter(|&day| d.weekday(day) == d.weekend_first);
            let longest = longest_run(weekends.map(|key| worked.binary_search(&key).is_err()));
            outside(longest, min, None)
        }),
    }
}
