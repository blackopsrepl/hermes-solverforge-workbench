//! Window rules: a measure of an employee's shifts per period or rolling window.

use super::Roster;
use crate::derived::{outside, Derived, Measure, ShiftInfo, WindowRule};
use crate::time::DAY;

/// Weighted deviation of every window rule matching `select`, for employee `e`.
pub fn window_penalty(roster: &Roster, e: usize, select: impl Fn(&WindowRule) -> bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let mut total = 0;
    for rule in d.employees[e].window_rules.iter().filter(|r| select(r)) {
        let items: Vec<&ShiftInfo> = roster
            .iter(d)
            .map(|(_, info)| info)
            .filter(|info| rule.shifts.accepts(&info.tags))
            .collect();
        let deviation: i64 = rule
            .windows
            .iter()
            .map(|&window| window_deviation(d, e, rule, &items, window))
            .sum();
        total += deviation * rule.weight;
    }
    total
}

/// Selects rules by measure, window type and level.
pub fn rules(measure: Measure, rolling: bool, preferred: bool) -> impl Fn(&WindowRule) -> bool {
    move |rule| rule.measure == measure && rule.rolling == rolling && rule.preferred == preferred
}

fn members<'a>(
    rule: &WindowRule,
    items: &[&'a ShiftInfo],
    (start, end): (i64, i64),
) -> Vec<&'a ShiftInfo> {
    let lo = items.partition_point(|info| info.start < start);
    let hi = items.partition_point(|info| info.start < end);
    items[lo..hi]
        .iter()
        .copied()
        .filter(|info| !rule.whole_shift || info.end <= end)
        .collect()
}

fn distinct(values: impl Iterator<Item = i64>) -> Vec<i64> {
    let mut out: Vec<i64> = values.collect();
    out.sort_unstable();
    out.dedup();
    out
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
    items: &[&ShiftInfo],
    window: (i64, i64),
) -> i64 {
    let inside = || members(rule, items, window);
    let (min, max) = (rule.min, rule.max);
    match rule.measure {
        Measure::Minutes => outside(inside().iter().map(|i| i.minutes).sum(), min, max),
        Measure::Logged => {
            let before: i64 = d.employees[e]
                .logged_before
                .iter()
                .filter(|(day, _)| (window.0..window.1).contains(&(day * DAY)))
                .map(|(_, minutes)| minutes)
                .sum();
            outside(
                before + inside().iter().map(|i| i.logged).sum::<i64>(),
                min,
                max,
            )
        }
        Measure::Days => outside(
            distinct(inside().iter().map(|i| i.day)).len() as i64,
            min,
            max,
        ),
        Measure::Shifts => outside(inside().len() as i64, min, max),
        Measure::Weekends => {
            let weekends = distinct(
                inside()
                    .iter()
                    .filter(|i| d.is_weekend(i.day))
                    .map(|i| d.weekend_of(i.day)),
            );
            outside(weekends.len() as i64, min, max)
        }
        Measure::WeekendMinutes => {
            outside(inside().iter().map(|i| i.weekend_minutes).sum(), min, max)
        }
        Measure::ConsecutiveWeekends => {
            let worked = distinct(
                inside()
                    .iter()
                    .filter(|i| d.is_weekend(i.day))
                    .map(|i| d.weekend_of(i.day)),
            );
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
        }
        Measure::DaysOff => {
            let worked = distinct(inside().iter().map(|i| i.day));
            let off = days_of(d, window)
                .filter(|&day| rule.weekdays & (1 << d.weekday(day)) != 0)
                .filter(|day| worked.binary_search(day).is_err())
                .count() as i64;
            outside(off, min, max)
        }
        Measure::ConsecutiveDaysOff => {
            let worked = distinct(inside().iter().map(|i| i.day));
            let longest =
                longest_run(days_of(d, window).map(|day| worked.binary_search(&day).is_err()));
            outside(longest, min, None)
        }
        Measure::MinutesOff => {
            let (start, end) = window;
            let mut cursor = start;
            let mut longest = 0;
            for info in items.iter().filter(|i| i.start < end && i.end > start) {
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
                    .iter()
                    .filter(|i| i.tags.binary_search(category).is_ok())
                    .count() as i64;
                outside(n, min, max)
            })
            .sum(),
        Measure::DistinctTags => {
            let used = distinct(
                inside()
                    .iter()
                    .flat_map(|i| i.tags.iter())
                    .filter(|tag| rule.tags.binary_search(tag).is_ok())
                    .map(|&tag| i64::from(tag)),
            );
            outside(used.len() as i64, min, max)
        }
        Measure::StartSpread => {
            let times: Vec<i64> = inside().iter().map(|i| i.start.rem_euclid(DAY)).collect();
            match (times.iter().min(), times.iter().max()) {
                (Some(lo), Some(hi)) => outside(hi - lo, min, max),
                _ => 0,
            }
        }
        Measure::ConsecutiveWeekendsOff => {
            let worked = distinct(
                inside()
                    .iter()
                    .filter(|i| d.is_weekend(i.day))
                    .map(|i| d.weekend_of(i.day)),
            );
            let weekends = days_of(d, window).filter(|&day| d.weekday(day) == d.weekend_first);
            let longest = longest_run(weekends.map(|key| worked.binary_search(&key).is_err()));
            outside(longest, min, None)
        }
    }
}
