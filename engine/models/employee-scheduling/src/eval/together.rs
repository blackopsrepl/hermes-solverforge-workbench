//! Working together: pairings between employees and teams, scored per day
//! bucket from the shifts that start on that day.

use super::Members;
use crate::derived::{ShiftInfo, Want};
use crate::time::overlap;

fn meets(a: &ShiftInfo, b: &ShiftInfo) -> bool {
    overlap(a.start, a.end, b.start, b.end) > 0
}

/// Pairing scores for stance `want` over shifts starting on `day`: a missing
/// partner for required pairings, a present partner otherwise. Soft stances
/// are multiplied by the employee's priority.
pub fn colleagues(m: &Members, day: usize, want: Want) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    let all = || m.iter(d);
    let mut total = 0;
    for (s, a, info) in all().filter(|(_, _, i)| i.day == day as i64) {
        let employee = &d.employees[a];
        for rule in employee.colleagues.iter().filter(|r| r.want == want) {
            if !rule.on_tags.is_empty()
                && !rule
                    .on_tags
                    .iter()
                    .any(|t| info.tags.binary_search(t).is_ok())
            {
                continue;
            }
            let together = all().any(|(t, b, other)| {
                t != s && rule.partners.binary_search(&(b as u32)).is_ok() && meets(info, other)
            });
            let hit = match want {
                Want::Required => !together,
                _ => together,
            };
            if hit {
                total += if matches!(want, Want::Required | Want::Barred) {
                    1
                } else {
                    employee.priority
                };
            }
        }
    }
    total
}

/// Which team score to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamScore {
    /// Overlapping shifts of two members share no tag of the team's tag type.
    SplitTags,
    /// A member's shift has no lead working at the same time.
    HelperWithoutLead,
    /// A lead's shift has another member working at the same time.
    LeadWithTeam,
    /// Distinct shift times per tag beyond the team's daily maximum.
    ShiftsPerDay,
}

/// Team scores over the shifts starting on `day`.
pub fn teams(m: &Members, day: usize, score: TeamScore) -> i64 {
    let Some(d) = m.derived() else {
        return 0;
    };
    if d.teams.is_empty() {
        return 0;
    }
    let all = || m.iter(d);
    let day = day as i64;
    let mut total = 0;
    if score == TeamScore::ShiftsPerDay {
        for team in d.teams.iter().filter(|t| t.max_shifts_per_day.is_some()) {
            let max = team.max_shifts_per_day.unwrap_or(i64::MAX);
            for tag in &team.tags {
                let mut times: Vec<(i64, i64)> = all()
                    .filter(|(_, e, i)| {
                        i.day == day
                            && team.everyone.binary_search(&(*e as u32)).is_ok()
                            && i.tags.binary_search(tag).is_ok()
                    })
                    .map(|(_, _, i)| (i.start, i.end))
                    .collect();
                times.sort_unstable();
                times.dedup();
                total += (times.len() as i64 - max).max(0);
            }
        }
        return total;
    }
    for (s, a, info) in all().filter(|(_, _, i)| i.day == day) {
        for &t in &d.employees[a].teams {
            let team = &d.teams[t as usize];
            let is_lead = team.leads.binary_search(&(a as u32)).is_ok();
            let others = all().filter(|&(o, b, other)| {
                o != s
                    && b != a
                    && team.everyone.binary_search(&(b as u32)).is_ok()
                    && meets(info, other)
            });
            match score {
                TeamScore::SplitTags => {
                    if team.tags.is_empty() {
                        continue;
                    }
                    let shared = |other: &ShiftInfo| {
                        team.tags.iter().any(|tag| {
                            info.tags.binary_search(tag).is_ok()
                                && other.tags.binary_search(tag).is_ok()
                        })
                    };
                    total += others
                        .filter(|&(o, _, other)| {
                            (other.start, o) > (info.start, s) && !shared(other)
                        })
                        .count() as i64;
                }
                TeamScore::HelperWithoutLead => {
                    let mut others = others;
                    if !is_lead
                        && !others.any(|(_, b, _)| team.leads.binary_search(&(b as u32)).is_ok())
                    {
                        total += 1;
                    }
                }
                TeamScore::LeadWithTeam => {
                    let mut others = others;
                    if is_lead && others.next().is_some() {
                        total += 1;
                    }
                }
                TeamScore::ShiftsPerDay => {}
            }
        }
    }
    total
}
