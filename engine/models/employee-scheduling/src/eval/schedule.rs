//! Schedule-wide scores: per day bucket and per schedule-wide window.

use super::Members;
use crate::derived::{outside, GlobalKind};
use crate::time::DAY;

/// Shifts beyond each concurrency limit, summed over the moments a matching
/// shift starts on `day`.
pub fn concurrency_excess(members: &Members, day: usize) -> i64 {
    let Some(d) = members.derived() else {
        return 0;
    };
    let (day_start, day_end) = (day as i64 * DAY, (day as i64 + 1) * DAY);
    let mut total = 0;
    for rule in &d.concurrency {
        let matching: Vec<_> = members
            .iter(d)
            .filter(|(_, _, info)| rule.shifts.accepts(&info.tags))
            .collect();
        for (_, _, starting) in &matching {
            let t = starting.start;
            if t < day_start || t >= day_end || t < rule.from || t >= rule.to {
                continue;
            }
            let active = matching
                .iter()
                .filter(|(_, _, info)| info.start <= t && t < info.end)
                .count() as i64;
            total += (active - rule.max).max(0);
        }
    }
    total
}

/// Weighted deviation of a schedule-wide window of `kind` and level.
pub fn window_deviation(
    members: &Members,
    window: usize,
    kind: GlobalKind,
    preferred: bool,
) -> i64 {
    let Some(d) = members.derived() else {
        return 0;
    };
    let rule = &d.windows[window];
    if rule.kind != kind || rule.preferred != preferred {
        return 0;
    }
    let value = match kind {
        GlobalKind::Shifts => members
            .iter(d)
            .filter(|(_, e, _)| {
                rule.employees
                    .as_ref()
                    .is_none_or(|f| f.accepts(&d.employees[*e].tags))
            })
            .count() as i64,
        GlobalKind::DistinctTags => {
            let mut used: Vec<u32> = members
                .iter(d)
                .flat_map(|(_, _, info)| info.tags.iter().copied())
                .filter(|tag| rule.tags.binary_search(tag).is_ok())
                .collect();
            used.sort_unstable();
            used.dedup();
            used.len() as i64
        }
        GlobalKind::Cost => members
            .iter(d)
            .filter(|(_, e, _)| {
                rule.employees
                    .as_ref()
                    .is_none_or(|f| f.accepts(&d.employees[*e].tags))
            })
            .map(|(_, e, info)| {
                let group = d.employees[e].cost_group;
                rule.rates
                    .iter()
                    .find(|rate| {
                        rate.employee_group.is_none_or(|g| Some(g) == group)
                            && rate.shift_group.is_none_or(|g| Some(g) == info.cost_group)
                    })
                    .map_or(0, |rate| (info.minutes * rate.cents + 30) / 60)
            })
            .sum(),
    };
    if kind == GlobalKind::Cost && rule.min.is_none() && rule.max.is_none() {
        return value * rule.weight;
    }
    outside(value, rule.min, rule.max) * rule.weight
}
