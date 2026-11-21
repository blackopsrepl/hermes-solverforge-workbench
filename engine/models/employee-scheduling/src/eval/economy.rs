//! Demand coverage, alternative shifts, activation, fairness and disruption.

use super::{Members, Roster};
use crate::derived::{Derived, ShiftInfo, Slot};

/// What to measure on demand slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// Staff short of the minimum.
    Under,
    /// Staff beyond the maximum.
    Over,
    /// Square of the staff beyond the minimum, any level: minimising the sum
    /// spreads surplus staff evenly over the slots.
    Surplus,
}

/// Coverage of the demand slots starting on `day`, for slots of the given
/// level. A shift covers a slot when it spans all of it and none of its
/// breaks falls inside it.
pub fn demand(m: &Members, day: usize, coverage: Coverage, preferred: bool) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    let Some(slots) = d.slots_by_day.get(day) else {
        return 0;
    };
    let slots: Vec<&Slot> = slots.iter().map(|&i| &d.slots[i as usize]).collect();
    let staff = staff(d, m, &slots);
    let mut total = 0;
    for (slot, staff) in slots.iter().zip(staff) {
        if coverage != Coverage::Surplus && slot.preferred != preferred {
            continue;
        }
        total += match coverage {
            Coverage::Under => slot.min.map_or(0, |min| (min - staff).max(0)),
            Coverage::Over => slot.max.map_or(0, |max| (staff - max).max(0)),
            Coverage::Surplus => slot.min.map_or(0, |min| (staff - min).max(0).pow(2)),
        };
    }
    total
}

/// Whether `info`, worked by employee `e`, spans `slot` and is staff it counts.
fn spans(d: &Derived, slot: &Slot, e: usize, info: &ShiftInfo) -> bool {
    info.start <= slot.start
        && info.end >= slot.end
        && slot.shifts.accepts(&info.tags)
        && slot
            .employees
            .as_ref()
            .is_none_or(|f| f.accepts(&d.employees[e].tags))
}

/// Staff each slot counts, with every break placed: fixed breaks where they
/// are, floating ones where they leave the fewest slots short.
fn staff(d: &Derived, m: &Members, slots: &[&Slot]) -> Vec<i64> {
    let mut rows: Vec<(usize, usize, &ShiftInfo)> = m.iter(d).collect();
    rows.sort_unstable_by_key(|row| row.0);
    let mut staff = vec![0i64; slots.len()];
    for &(_, e, info) in &rows {
        for (j, slot) in slots.iter().enumerate() {
            if spans(d, slot, e, info) {
                staff[j] += 1;
            }
        }
    }
    let take = |staff: &mut Vec<i64>, e: usize, info: &ShiftInfo, start: i64, end: i64| {
        for (j, slot) in slots.iter().enumerate() {
            if start < slot.end && end > slot.start && spans(d, slot, e, info) {
                staff[j] -= 1;
            }
        }
    };
    // Fixed breaks first, so floating ones see where staff is already short.
    for &(_, e, info) in &rows {
        for &(from, to, minutes) in &info.breaks {
            if minutes >= to - from {
                take(&mut staff, e, info, from, to);
            }
        }
    }
    for &(_, e, info) in &rows {
        for (start, end) in floating_breaks(info, |start, end| {
            // What a break over `start..end` costs: the required slots it
            // leaves short first, then the preferred ones.
            slots
                .iter()
                .zip(&staff)
                .filter(|(slot, _)| start < slot.end && end > slot.start && spans(d, slot, e, info))
                .map(|(slot, &count)| match slot.min {
                    Some(min) if count <= min && !slot.preferred => 1_000,
                    Some(min) if count <= min => 1,
                    _ => 0,
                })
                .sum()
        }) {
            take(&mut staff, e, info, start, end);
        }
    }
    staff
}

/// The floating breaks of `info` as `(start, end)`, each at the start, in
/// 15-minute steps through its window, that `cost` finds cheapest, the
/// earliest on ties.
pub fn floating_breaks(info: &ShiftInfo, mut cost: impl FnMut(i64, i64) -> i64) -> Vec<(i64, i64)> {
    info.breaks
        .iter()
        .filter(|&&(from, to, minutes)| minutes < to - from)
        .map(|&(from, to, minutes)| {
            let last = to - minutes;
            let start = (from..last)
                .step_by(15)
                .chain(std::iter::once(last))
                .min_by_key(|&start| (cost(start, start + minutes), start))
                .unwrap_or(from);
            (start, start + minutes)
        })
        .collect()
}

/// Assigned shifts of an alternatives group away from its count (beyond it
/// for `at most` groups).
pub fn alternatives_off(m: &Members, group: usize) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    let Some(&(count, at_most)) = d.alternatives.get(group) else {
        return 0;
    };
    let worked = m.len() as i64;
    if at_most {
        (worked - count).max(0)
    } else {
        (worked - count).abs()
    }
}

fn in_horizon<'a>(
    roster: &'a Roster,
    d: &'a Derived,
) -> impl Iterator<Item = (usize, &'a ShiftInfo)> + 'a {
    roster.iter(d).filter(|(_, info)| info.in_horizon)
}

/// Activation cost of an employee who works any shift in the horizon.
pub fn activation(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let employee = &d.employees[e];
    if employee.activation_cost > 0 && in_horizon(roster, d).next().is_some() {
        employee.activation_cost
    } else {
        0
    }
}

/// Square of the shifts an employee of a fill-up group works: rewarding it
/// concentrates work on fewer employees.
pub fn fill_up(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    if !d.employees[e].fill_up {
        return 0;
    }
    let n = in_horizon(roster, d).count() as i64;
    n * n
}

/// Sum over the employee's fairness rules balancing minutes (or shifts) of
/// the squared load, history included: minimising the sum of squares
/// spreads a fixed total evenly. Minutes count in hours.
pub fn unfairness(roster: &Roster, e: usize, minutes: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    d.employees[e]
        .fairness
        .iter()
        .filter(|rule| rule.minutes == minutes)
        .map(|rule| {
            let planned: i64 = in_horizon(roster, d)
                .filter(|(_, info)| rule.shifts.accepts(&info.tags))
                .map(|(_, info)| if rule.minutes { info.minutes } else { 1 })
                .sum();
            let load = rule.history + planned;
            if rule.minutes {
                load * load / 3600
            } else {
                load * load
            }
        })
        .sum()
}

/// The employee's published shifts the plan takes away, and the shifts it
/// gives them that were not published to them (only when replanning).
fn lost_and_gained<'a>(
    roster: &'a Roster,
    d: &'a Derived,
    e: usize,
) -> (Vec<&'a ShiftInfo>, Vec<&'a ShiftInfo>) {
    if !d.replanning {
        return (Vec::new(), Vec::new());
    }
    let assigned = roster.indexes();
    let lost = d.employees[e]
        .published_shifts
        .iter()
        .filter(|s| !assigned.contains(s))
        .map(|&s| &d.shifts[s as usize])
        .filter(|info| info.in_horizon)
        .collect();
    let gained = in_horizon(roster, d)
        .filter(|(_, info)| info.published != Some(e as u32))
        .map(|(_, info)| info)
        .collect();
    (lost, gained)
}

/// Published shifts the employee loses, times their priority.
pub fn shifts_lost(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    lost_and_gained(roster, d, e).0.len() as i64 * d.employees[e].priority
}

/// Shifts the employee gains that were not published to them, times their
/// priority.
pub fn shifts_gained(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    lost_and_gained(roster, d, e).1.len() as i64 * d.employees[e].priority
}

/// Tags of gained shifts that some lost shift of the employee carried.
pub fn gained_tags_kept(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let (lost, gained) = lost_and_gained(roster, d, e);
    if lost.is_empty() {
        return 0;
    }
    gained
        .iter()
        .flat_map(|g| g.tags.iter())
        .filter(|tag| lost.iter().any(|l| l.tags.binary_search(tag).is_ok()))
        .count() as i64
}

/// Per scoped disruption rule of the given level: the weight of lost
/// matching shifts no gained shift replaces, and the kept tags of those
/// that are replaced. A gained shift replaces a lost one when its start and
/// end are within the rule's minutes of it and it keeps the rule's tags.
fn scoped(roster: &Roster, e: usize, level: Option<bool>) -> (i64, i64) {
    let Some(d) = roster.derived() else {
        return (0, 0);
    };
    let (lost, gained) = lost_and_gained(roster, d, e);
    if lost.is_empty() {
        return (0, 0);
    }
    let (mut missing, mut kept) = (0, 0);
    for rule in d.employees[e]
        .disruption
        .iter()
        .filter(|r| level.is_none_or(|preferred| r.preferred == preferred))
    {
        let mut spare: Vec<&ShiftInfo> = gained
            .iter()
            .copied()
            .filter(|info| rule.shifts.accepts(&info.tags))
            .collect();
        for old in lost.iter().filter(|info| rule.shifts.accepts(&info.tags)) {
            let keep: Vec<u32> = rule
                .keep_tags
                .iter()
                .copied()
                .filter(|t| old.tags.binary_search(t).is_ok())
                .collect();
            let replacement = rule.within.and_then(|within| {
                spare.iter().position(|new| {
                    (new.start - old.start).abs() <= within
                        && (new.end - old.end).abs() <= within
                        && keep.iter().all(|t| new.tags.binary_search(t).is_ok())
                })
            });
            match replacement {
                Some(at) => {
                    spare.swap_remove(at);
                    kept += keep.len() as i64;
                }
                None => missing += rule.weight,
            }
        }
    }
    (missing, kept)
}

/// Weighted lost shifts without a close replacement, per level.
pub fn unreplaced(roster: &Roster, e: usize, preferred: bool) -> i64 {
    scoped(roster, e, Some(preferred)).0
}

/// Rule tags kept by close replacements of lost shifts.
pub fn replacement_tags_kept(roster: &Roster, e: usize) -> i64 {
    scoped(roster, e, None).1
}

/// Distance of activated employees per cost group from the groups' target
/// shares: the sum over groups of |activated - share of all activated|,
/// rounded to whole employees.
pub fn share_gap(m: &Members) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    if d.shares.is_empty() {
        return 0;
    }
    let mut seen = vec![false; d.employees.len()];
    let mut activated = vec![0i64; d.shares.len()];
    for (_, e, info) in m.iter(d) {
        if !info.in_horizon || seen[e] {
            continue;
        }
        seen[e] = true;
        if let Some(g) = d.employees[e].share_group {
            activated[g as usize] += 1;
        }
    }
    let all: i64 = activated.iter().sum();
    let weight: i64 = d.shares.iter().sum();
    if weight == 0 {
        return 0;
    }
    let gap: i64 = activated
        .iter()
        .zip(&d.shares)
        .map(|(&a, &share)| (a * weight - all * share).abs())
        .sum();
    (gap + weight / 2) / weight
}
