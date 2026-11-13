//! Rotations and patterns: overlaps, gaps between shifts, pairings, day and
//! multi-day patterns and rotations, all within one employee's roster.

use super::Roster;
use crate::derived::{outside, Derived, ShiftInfo, TagMatch, Want};
use crate::time::{overlap, DAY};

fn infos<'a>(roster: &'a Roster, d: &'a Derived) -> Vec<&'a ShiftInfo> {
    roster.iter(d).map(|(_, info)| info).collect()
}

/// Minutes in which two of the employee's shifts overlap, unless both pass
/// one of the employee's overlap-allowed filters.
pub fn overlap_minutes(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let allowed = &d.employees[e].overlap_allowed;
    let items = infos(roster, d);
    let mut total = 0;
    for (n, a) in items.iter().enumerate() {
        for b in items[n + 1..].iter().take_while(|b| b.start < a.end) {
            if allowed
                .iter()
                .any(|f| f.accepts(&a.tags) && f.accepts(&b.tags))
            {
                continue;
            }
            total += overlap(a.start, a.end, b.start, b.end);
        }
    }
    total
}

/// Rest-between (`split = false`, distance outside the range) or split-shift
/// (`split = true`, minutes under the largest gap) scores of one level.
pub fn gaps(roster: &Roster, e: usize, split: bool, preferred: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items = infos(roster, d);
    let mut total = 0;
    for rule in d.employees[e]
        .gaps
        .iter()
        .filter(|r| r.split == split && r.preferred == preferred)
    {
        let mut sum = 0;
        for pair in items.windows(2) {
            let (prev, next) = (pair[0], pair[1]);
            if !rule.before.accepts(&prev.tags) || !rule.after.accepts(&next.tags) {
                continue;
            }
            let gap = next.start - prev.end;
            if gap < 0 || rule.within.is_some_and(|within| gap > within) {
                continue;
            }
            sum += if split {
                rule.max.map_or(0, |max| (max - gap).max(0))
            } else {
                outside(gap, rule.min, rule.max)
            };
        }
        total += sum * rule.weight;
    }
    total
}

/// Daily pairing scores for rules with stance `want`: missing pairs for
/// required and preferred rules, present pairs for unwanted and barred ones.
pub fn pairings(roster: &Roster, e: usize, want: Want) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items: Vec<(usize, &ShiftInfo)> = roster.iter(d).collect();
    let mut total = 0;
    for rule in d.employees[e].pairings.iter().filter(|r| r.want == want) {
        let mut hits = 0;
        for &(i, s) in items.iter().filter(|(_, s)| rule.first.accepts(&s.tags)) {
            let paired = items.iter().any(|&(j, t)| {
                j != i && t.day == s.day + rule.offset && rule.paired.accepts(&t.tags)
            });
            let counts = match want {
                Want::Required | Want::Preferred => !paired,
                Want::Unwanted | Want::Barred => paired,
            };
            hits += i64::from(counts);
        }
        total += hits * rule.weight;
    }
    total
}

/// Whether `steps` can be matched by distinct shifts of `day`, in order when `ordered`.
fn day_matches(steps: &[TagMatch], day: &[&ShiftInfo], ordered: bool) -> bool {
    if ordered {
        let mut next = 0;
        for step in steps {
            match day[next..].iter().position(|s| step.accepts(&s.tags)) {
                Some(at) => next += at + 1,
                None => return false,
            }
        }
        true
    } else {
        fn assign(steps: &[TagMatch], day: &[&ShiftInfo], used: &mut Vec<bool>) -> bool {
            let Some((step, rest)) = steps.split_first() else {
                return true;
            };
            for (n, shift) in day.iter().enumerate() {
                if !used[n] && step.accepts(&shift.tags) {
                    used[n] = true;
                    if assign(rest, day, used) {
                        return true;
                    }
                    used[n] = false;
                }
            }
            false
        }
        assign(steps, day, &mut vec![false; day.len()])
    }
}

/// Single-day pattern scores for rules with stance `want`: per day, a broken
/// pattern for required rules, a matched pattern otherwise.
pub fn day_patterns(roster: &Roster, e: usize, want: Want) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items = infos(roster, d);
    let mut days: Vec<&[&ShiftInfo]> = Vec::new();
    let mut from = 0;
    for n in 1..=items.len() {
        if n == items.len() || items[n].day != items[from].day {
            days.push(&items[from..n]);
            from = n;
        }
    }
    let mut total = 0;
    for rule in d.employees[e]
        .day_patterns
        .iter()
        .filter(|r| r.want == want)
    {
        let mut hits = 0;
        for day in &days {
            let matched = day_matches(&rule.steps, day, rule.ordered);
            hits += i64::from(match want {
                Want::Required => {
                    !matched
                        && day
                            .iter()
                            .any(|s| rule.steps.iter().any(|step| step.accepts(&s.tags)))
                }
                _ => matched,
            });
        }
        total += hits * rule.weight;
    }
    total
}

/// Multi-day pattern scores for rules with stance `want`: per start day, a
/// broken pattern for required rules (its first day worked, the rest not as
/// asked), a matched pattern otherwise.
pub fn multi_day_patterns(roster: &Roster, e: usize, want: Want) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items = infos(roster, d);
    let worked = |day: i64, filter: &TagMatch| {
        let lo = items.partition_point(|s| s.day < day);
        items[lo..]
            .iter()
            .take_while(|s| s.day == day)
            .any(|s| filter.accepts(&s.tags))
    };
    let mut total = 0;
    for rule in d.employees[e].multi_day.iter().filter(|r| r.want == want) {
        let len = rule.days.len() as i64;
        let mut hits = 0;
        for start in 0..=(d.days - len) {
            if start * DAY >= d.horizon_end || (start + len) * DAY <= d.horizon_start {
                continue;
            }
            let day_ok = |k: usize| {
                let (work, filter) = &rule.days[k];
                worked(start + k as i64, filter) == *work
            };
            let matched = (0..rule.days.len()).all(day_ok);
            hits += i64::from(match want {
                Want::Required => day_ok(0) && !matched,
                _ => matched,
            });
        }
        total += hits * rule.weight;
    }
    total
}

/// The first rotation group a shift belongs to.
fn group_of(groups: &[TagMatch], shift: &ShiftInfo) -> Option<usize> {
    groups.iter().position(|g| g.accepts(&shift.tags))
}

/// Weekly rotation scores of one level: shifts outside the week's group,
/// plus weeks that do not move on to the next group.
pub fn weekly_rotation(roster: &Roster, e: usize, preferred: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items = infos(roster, d);
    let mut total = 0;
    for rule in d.employees[e]
        .rotations
        .iter()
        .filter(|r| r.preferred == preferred && !r.weeks.is_empty())
    {
        let mut misses = 0;
        let mut previous: Option<usize> = None;
        for &(start, end) in &rule.weeks {
            let week: Vec<usize> = items
                .iter()
                .filter(|s| s.start >= start && s.start < end)
                .filter_map(|s| group_of(&rule.groups, s))
                .collect();
            let Some(&chosen) = week.first() else {
                previous = None;
                continue;
            };
            misses += week.iter().filter(|&&g| g != chosen).count() as i64;
            if previous.is_some_and(|p| (p + 1) % rule.groups.len() != chosen) {
                misses += 1;
            }
            previous = Some(chosen);
        }
        total += misses * rule.weight;
    }
    total
}

/// Block rotation scores of one level: block lengths in days outside
/// `[min, max]` (required rules only limit the maximum).
pub fn rotation_blocks(roster: &Roster, e: usize, preferred: bool) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let items = infos(roster, d);
    let mut total = 0;
    for rule in d.employees[e]
        .rotations
        .iter()
        .filter(|r| r.preferred == preferred && r.weeks.is_empty())
    {
        let mut blocks: Vec<(usize, i64, i64)> = Vec::new();
        for s in &items {
            let Some(group) = group_of(&rule.groups, s) else {
                continue;
            };
            match blocks.last_mut() {
                Some((g, _, last)) if *g == group => *last = s.day,
                _ => blocks.push((group, s.day, s.day)),
            }
        }
        let min = if preferred { rule.min } else { None };
        let deviation: i64 = blocks
            .iter()
            .filter(|(_, first, last)| {
                first * DAY < d.horizon_end && (last + 1) * DAY > d.horizon_start
            })
            .map(|(_, first, last)| outside(last - first + 1, min, rule.max))
            .sum();
        total += deviation * rule.weight;
    }
    total
}

/// Minutes missing between successive shifts at different locations, per
/// the employee's travel rules (the largest requirement of the rules that
/// match the later shift).
pub fn location_change_shortfall(roster: &Roster, e: usize) -> i64 {
    let Some(d) = roster.derived() else {
        return 0;
    };
    let rules = &d.employees[e].travel;
    if rules.iter().all(|r| r.min_change.is_none()) {
        return 0;
    }
    let items = infos(roster, d);
    let mut total = 0;
    for pair in items.windows(2) {
        let (prev, next) = (pair[0], pair[1]);
        if prev.location_id.is_none()
            || next.location_id.is_none()
            || prev.location_id == next.location_id
        {
            continue;
        }
        let need = rules
            .iter()
            .filter(|r| r.shifts.accepts(&next.tags))
            .filter_map(|r| r.min_change)
            .max()
            .unwrap_or(0);
        total += (need - (next.start - prev.end)).max(0);
    }
    total
}
