//! Demand coverage, alternative shifts, activation, fairness and disruption.

use super::{Members, Roster};
use crate::derived::{Derived, ShiftInfo};

/// Staff short of (`under`) or beyond (`!under`) the demand slots starting
/// on `day`, for slots of the given level. A shift covers a slot when it
/// spans all of it.
pub fn demand(m: &Members, day: usize, under: bool, preferred: bool) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    let Some(slots) = d.slots_by_day.get(day) else {
        return 0;
    };
    let mut total = 0;
    for &index in slots {
        let slot = &d.slots[index as usize];
        if slot.preferred != preferred {
            continue;
        }
        let staff = m
            .iter(d)
            .filter(|(_, e, info)| {
                info.start <= slot.start
                    && info.end >= slot.end
                    && slot.shifts.accepts(&info.tags)
                    && slot
                        .employees
                        .as_ref()
                        .is_none_or(|f| f.accepts(&d.employees[*e].tags))
            })
            .count() as i64;
        total += if under {
            slot.min.map_or(0, |min| (min - staff).max(0))
        } else {
            slot.max.map_or(0, |max| (staff - max).max(0))
        };
    }
    total
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

/// Sum over the employee's fairness rules of the squared load (history
/// plus planned): minimising the sum of squares spreads a fixed total
/// evenly. Minutes count in hours.
pub fn unfairness(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    d.employees[e]
        .fairness
        .iter()
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

/// Published shifts of the employee that the plan takes away and no new
/// shift makes good, weighted per disruption rule of the given level. A new
/// shift makes good a lost one when its start and end are within the rule's
/// minutes of it and it keeps the rule's tags.
pub fn disruption(roster: &Roster, e: usize, preferred: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let employee = &d.employees[e];
    if employee.published_shifts.is_empty() {
        return 0;
    }
    let assigned: Vec<u32> = roster.indexes().to_vec();
    let mut total = 0;
    for rule in employee
        .disruption
        .iter()
        .filter(|r| r.preferred == preferred)
    {
        let mut spare: Vec<&ShiftInfo> = roster
            .iter(d)
            .filter(|(s, info)| {
                info.in_horizon
                    && info.published != Some(e as u32)
                    && rule.shifts.accepts(&info.tags)
                    && !employee.published_shifts.contains(&(*s as u32))
            })
            .map(|(_, info)| info)
            .collect();
        for &s in &employee.published_shifts {
            let lost = &d.shifts[s as usize];
            if !lost.in_horizon || !rule.shifts.accepts(&lost.tags) || assigned.contains(&s) {
                continue;
            }
            let replacement = rule.within.and_then(|within| {
                spare.iter().position(|new| {
                    (new.start - lost.start).abs() <= within
                        && (new.end - lost.end).abs() <= within
                        && rule
                            .keep_tags
                            .iter()
                            .filter(|t| lost.tags.binary_search(t).is_ok())
                            .all(|t| new.tags.binary_search(t).is_ok())
                })
            });
            match replacement {
                Some(at) => {
                    spare.swap_remove(at);
                }
                None => total += rule.weight,
            }
        }
    }
    total
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
