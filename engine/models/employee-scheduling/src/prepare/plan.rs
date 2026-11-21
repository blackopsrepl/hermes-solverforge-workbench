//! `prepare`: validate a dataset and derive its fact tables.

use std::sync::Arc;

use solverforge_workbench_host::Issue;

use std::collections::HashMap;

use super::contracts::{compile, Compiled};
use super::schedule_rules::compile_schedule_rules;
use super::{Builder, Calendar};
use crate::derived::{Ctx, Derived, EmployeeInfo, ShiftInfo};
use crate::domain::{Choice, Day, Plan, Whole, Window};
use crate::time::{overlap, parse_date_time, DAY};

/// Longest shift, in calendar days touched, that the day buckets support.
pub const MAX_SHIFT_DAYS: i64 = 3;

/// Validates `plan`, resolves its ids and fills every derived field.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    let mut b = Builder::new();

    let horizon = (
        parse_date_time(&plan.schedule.start),
        parse_date_time(&plan.schedule.end),
    );
    let (Some(horizon_start), Some(horizon_end)) = horizon else {
        b.error(
            "invalid_schedule",
            format!(
                "the schedule runs from {:?} to {:?}; both must be YYYY-MM-DDTHH:MM",
                plan.schedule.start, plan.schedule.end
            ),
        );
        return b.issues;
    };
    if horizon_end <= horizon_start {
        b.error(
            "empty_schedule",
            "the schedule ends at or before it starts".into(),
        );
        return b.issues;
    }

    // Plan day 0 is the earliest day touched by the horizon or any shift.
    let earliest = plan
        .shifts
        .iter()
        .filter_map(|shift| parse_date_time(&shift.start))
        .chain([horizon_start])
        .min()
        .unwrap_or(horizon_start);
    let latest = plan
        .shifts
        .iter()
        .filter_map(|shift| parse_date_time(&shift.end))
        .chain([horizon_end])
        .max()
        .unwrap_or(horizon_end);
    let origin_day = earliest.div_euclid(DAY);
    b.origin_minutes = origin_day * DAY;

    if plan.employees.is_empty() {
        b.warning("no_employees", "the dataset has no employees".into());
    }
    if plan.shifts.is_empty() {
        b.warning("no_shifts", "the dataset has no shifts".into());
    }

    for (index, employee) in plan.employees.iter_mut().enumerate() {
        employee.index = index;
        if b.employee_ids.insert(employee.id.clone(), index).is_some() {
            b.error(
                "duplicate_employee",
                format!("employee id {:?} is used more than once", employee.id),
            );
        }
    }
    for (index, shift) in plan.shifts.iter_mut().enumerate() {
        shift.index = index;
        if b.shift_ids.insert(shift.id.clone(), index).is_some() {
            b.error(
                "duplicate_shift",
                format!("shift id {:?} is used more than once", shift.id),
            );
        }
    }

    let mut weekend_mask = 0u8;
    for day in &plan.schedule.weekend {
        weekend_mask |= 1 << day.number();
    }
    let weekend_first = (0..7u8)
        .find(|&w| weekend_mask & (1 << w) != 0 && weekend_mask & (1 << ((w + 6) % 7)) == 0)
        .unwrap_or(5);
    if weekend_mask != 0 && weekend_mask != 0x7f {
        let len = weekend_mask.count_ones() as u8;
        let run = (0..len).fold(0u8, |m, k| m | 1 << ((weekend_first + k) % 7));
        if run != weekend_mask {
            b.error(
                "weekend_not_consecutive",
                "the weekend days must follow each other".into(),
            );
        }
    }

    let h_start = horizon_start - b.origin_minutes;
    let h_end = horizon_end - b.origin_minutes;
    let days = (latest - b.origin_minutes + DAY - 1).div_euclid(DAY);
    let mut custom = HashMap::new();
    for period in &plan.schedule.periods {
        if matches!(
            period.id.as_str(),
            "day" | "week" | "month" | "year" | "schedule"
        ) {
            b.error(
                "custom_period_reserved",
                format!(
                    "custom period {:?} reuses a built-in period name",
                    period.id
                ),
            );
        }
        let mut windows = Vec::new();
        for range in &period.ranges {
            let what = || format!("custom period {}", period.id);
            let (Some(from), Some(to)) = (b.day(&range.from, what), b.day(&range.to, what)) else {
                continue;
            };
            if to < from {
                b.error(
                    "period_range_reversed",
                    format!(
                        "custom period {} has a range ending before it starts",
                        period.id
                    ),
                );
                continue;
            }
            windows.push((from * DAY, (to + 1) * DAY));
        }
        windows.sort_unstable();
        custom.insert(period.id.clone(), windows);
    }
    for tag in &plan.tags {
        let id = b.intern(&tag.id);
        b.tag_types.entry(tag.kind.clone()).or_default().push(id);
    }
    let calendar = Calendar {
        origin: origin_day,
        days,
        horizon_start: h_start,
        horizon_end: h_end,
        week_start: plan.schedule.week_start.number(),
        custom,
    };
    let mut contracts: HashMap<String, Compiled> = HashMap::new();
    for contract in &plan.contracts {
        let compiled = compile(&mut b, &calendar, contract);
        if contracts.insert(contract.id.clone(), compiled).is_some() {
            b.error(
                "duplicate_contract",
                format!("contract id {:?} is used more than once", contract.id),
            );
        }
    }

    let mut shares = Vec::new();
    let mut groups: HashMap<String, (i64, bool, Option<u32>)> = HashMap::new();
    for group in &plan.cost_groups {
        let share = group.share.map(|share| {
            shares.push(share.max(0));
            shares.len() as u32 - 1
        });
        if group.activation_cost < 0 || group.share.is_some_and(|s| s < 0) {
            b.error(
                "negative_weight",
                format!("cost group {} has a negative cost or share", group.id),
            );
        }
        if groups
            .insert(
                group.id.clone(),
                (group.activation_cost.max(0), group.fill_up, share),
            )
            .is_some()
        {
            b.error(
                "duplicate_cost_group",
                format!("cost group id {:?} is used more than once", group.id),
            );
        }
    }
    for (n, rule) in plan.schedule_rules.fairness.iter().enumerate() {
        for id in rule.history.keys() {
            if !b.employee_ids.contains_key(id) {
                b.error(
                    "unknown_employee",
                    format!(
                        "fairness rule #{} has history for unknown employee {id:?}",
                        n + 1
                    ),
                );
            }
        }
    }
    let alternative_ids: HashMap<&str, u32> = plan
        .alternatives
        .iter()
        .enumerate()
        .map(|(i, a)| (a.id.as_str(), i as u32))
        .collect();
    for a in &plan.alternatives {
        if a.count < 0 {
            b.error(
                "rule_negative_limit",
                format!("alternatives {} has a negative count", a.id),
            );
        }
    }
    let employee_tags: Vec<Vec<u32>> = plan
        .employees
        .iter()
        .map(|employee| b.tag_set(&employee.tags))
        .collect();
    let employees: Vec<EmployeeInfo> = plan
        .employees
        .iter()
        .map(|employee| {
            let what = format!("employee {}", employee.id);
            if employee.priority < 1 {
                b.error(
                    "invalid_priority",
                    format!(
                        "{what} has priority {}; it must be at least 1",
                        employee.priority
                    ),
                );
            }
            let mut rules = Compiled::default();
            for id in &employee.contracts {
                match contracts.get(id) {
                    Some(contract) => rules.extend_scaled(contract, employee.priority.max(1)),
                    None => b.error(
                        "unknown_contract",
                        format!("{what} names contract {id:?}, which does not exist"),
                    ),
                }
            }
            let mut requested_off: Vec<i64> = employee
                .days_off
                .iter()
                .filter_map(|date| b.day(date, || format!("{what} day off")))
                .collect();
            requested_off.sort_unstable();
            requested_off.dedup();
            let logged_before = employee
                .logged_before
                .iter()
                .filter_map(|entry| {
                    let day = b.day(&entry.date, || format!("{what} logged minutes"))?;
                    Some((day, entry.minutes))
                })
                .collect();
            let mut colleagues = Vec::new();
            for pairing in &employee.pairings {
                let partners: Vec<u32> = match (&pairing.with, pairing.with_tags.is_empty()) {
                    (Some(id), true) => {
                        b.employee_list(std::slice::from_ref(id), || format!("{what} pairing"))
                    }
                    (None, false) => {
                        let wanted = b.tag_set(&pairing.with_tags);
                        employee_tags
                            .iter()
                            .enumerate()
                            .filter(|(i, tags)| {
                                *i != employee.index
                                    && wanted.iter().any(|t| tags.binary_search(t).is_ok())
                            })
                            .map(|(i, _)| i as u32)
                            .collect()
                    }
                    _ => {
                        b.error(
                            "pairing_target_invalid",
                            format!(
                                "{what} has a pairing that needs exactly one of with or withTags"
                            ),
                        );
                        continue;
                    }
                };
                if partners.contains(&(employee.index as u32)) {
                    b.error("pairing_with_self", format!("{what} is paired with itself"));
                    continue;
                }
                colleagues.push(crate::derived::Colleagues {
                    want: match pairing.stance {
                        crate::wire::Stance::Required => crate::derived::Want::Required,
                        crate::wire::Stance::Preferred => crate::derived::Want::Preferred,
                        crate::wire::Stance::Unwanted => crate::derived::Want::Unwanted,
                        crate::wire::Stance::Barred => crate::derived::Want::Barred,
                    },
                    partners,
                    on_tags: b.tag_set(&pairing.on_tags),
                });
            }
            let (activation_cost, fill_up, share_group) = employee
                .cost_group
                .as_ref()
                .and_then(|g| groups.get(g).copied())
                .unwrap_or((0, false, None));
            let mut fairness = Vec::new();
            for rule in &plan.schedule_rules.fairness {
                if let Some(filter) = &rule.employees {
                    if !b.tag_match(filter).accepts(&employee_tags[employee.index]) {
                        continue;
                    }
                }
                fairness.push(crate::derived::FairShare {
                    minutes: rule.balance == crate::wire::Balance::Minutes,
                    shifts: b.tag_match(&rule.shifts),
                    history: rule.history.get(&employee.id).copied().unwrap_or(0),
                });
            }
            EmployeeInfo {
                cost_group: employee.cost_group.as_ref().map(|g| b.intern(g)),
                activation_cost,
                fill_up,
                share_group,
                published_shifts: Vec::new(),
                disruption: rules.disruption,
                fairness,
                colleagues,
                teams: Vec::new(),
                home: employee.home.map(|p| (p.lat, p.lon)),
                travel: rules.travel,
                window_rules: rules.windows,
                day_runs: rules.day_runs,
                shift_runs: rules.shift_runs,
                near_day_off: rules.near_day_off,
                overlap_allowed: rules.overlap_allowed,
                gaps: rules.gaps,
                pairings: rules.pairings,
                day_patterns: rules.day_patterns,
                multi_day: rules.multi_day,
                rotations: rules.rotations,
                requested_off,
                logged_before,
                required_shifts: employee.required_shifts.as_ref().map(|f| b.tag_match(f)),
                preferred_shifts: employee.preferred_shifts.as_ref().map(|f| b.tag_match(f)),
                tags: b.tag_set(&employee.tags),
                priority: employee.priority.max(1),
                skills: b.grants(&employee.skills, &what),
                avoid_risks: b.tag_set(&employee.avoid_risks),
                unavailable: b.spans(&employee.unavailable, &format!("{what} unavailable time")),
                available_only: b
                    .spans(&employee.available_only, &format!("{what} available time")),
                preferred_times: b
                    .spans(&employee.preferred_times, &format!("{what} preferred time")),
                unwanted_times: b.spans(&employee.unwanted_times, &format!("{what} unwanted time")),
            }
        })
        .collect();

    let mut places: HashMap<(u64, u64), u32> = HashMap::new();
    let mut shifts = Vec::with_capacity(plan.shifts.len());
    for shift in plan.shifts.iter_mut() {
        let what = format!("shift {}", shift.id);
        let start = b.minutes(&shift.start, || what.clone()).unwrap_or(0);
        let end = b.minutes(&shift.end, || what.clone()).unwrap_or(start + 1);
        if end <= start {
            b.error(
                "shift_not_after_start",
                format!("{what} ends at or before it starts"),
            );
        }
        let end = end.max(start + 1);
        let (day, last_day) = (start.div_euclid(DAY), (end - 1).div_euclid(DAY));
        if last_day - day >= MAX_SHIFT_DAYS {
            b.error(
                "shift_too_long",
                format!("{what} touches more than {MAX_SHIFT_DAYS} calendar days"),
            );
        }
        if shift.importance < 0 {
            b.error(
                "negative_importance",
                format!("{what} has a negative importance"),
            );
        }
        shift.employee_idx = match &shift.employee {
            None => None,
            Some(id) => match b.employee_ids.get(id) {
                Some(&index) => Some(index),
                None => {
                    b.error(
                        "unknown_employee",
                        format!("{what} is assigned to employee {id:?}, which does not exist"),
                    );
                    None
                }
            },
        };
        let in_horizon = start < h_end && end > h_start;
        if !in_horizon && !shift.pinned {
            b.warning(
                "outside_schedule_pinned",
                format!("{what} lies outside the schedule, so it is kept as given"),
            );
            shift.pinned = true;
        }
        let mut break_minutes = 0;
        let mut breaks = Vec::new();
        for brk in &shift.breaks {
            let from = b.minutes(&brk.from, || format!("{what} break"));
            let to = b.minutes(&brk.to, || format!("{what} break"));
            if let (Some(from), Some(to)) = (from, to) {
                if to <= from || from < start || to > end {
                    b.error(
                        "break_outside_shift",
                        format!("{what} has a break that is empty or outside the shift"),
                    );
                }
                let window = overlap(from, to, start, end);
                let length = match brk.minutes {
                    Some(m) if m <= 0 || m > to - from => {
                        b.error(
                            "bad_break_minutes",
                            format!("{what} has a break longer than its window or not above zero"),
                        );
                        window
                    }
                    Some(m) => m.min(window),
                    None => window,
                };
                break_minutes += length;
                breaks.push((from.max(start), to.min(end), length));
            }
        }
        let minutes = (end - start - break_minutes).max(0);
        if shift.logged_minutes.is_some_and(|m| m < 0) {
            b.error(
                "negative_logged_minutes",
                format!("{what} has negative logged minutes"),
            );
        }
        let weekend_minutes = (day..=last_day)
            .filter(|&d| weekend_mask & (1 << crate::time::weekday(origin_day + d)) != 0)
            .map(|d| overlap(d * DAY, (d + 1) * DAY, start, end))
            .sum();
        let tags = b.tag_set(&shift.tags);
        let preferred_employees = b.employee_list(&shift.preferred_employees, || {
            format!("{what} preferred employees")
        });
        let unwanted_employees = b.employee_list(&shift.unwanted_employees, || {
            format!("{what} unwanted employees")
        });
        let barred_employees = b.employee_list(&shift.barred_employees, || {
            format!("{what} barred employees")
        });
        if let Some(employee) = shift.employee_idx {
            if shift.pinned && barred_employees.binary_search(&(employee as u32)).is_ok() {
                b.warning(
                    "pinned_barred_employee",
                    format!("{what} is pinned to an employee it bars"),
                );
            }
        }
        let location = shift.location.map(|p| (p.lat, p.lon));
        let location_id = location.map(|(lat, lon)| {
            let next = places.len() as u32;
            *places.entry((lat.to_bits(), lon.to_bits())).or_insert(next)
        });
        let alternative = match &shift.alternative_of {
            Some(id) => match alternative_ids.get(id.as_str()) {
                Some(&index) => Some(index),
                None => {
                    b.error(
                        "unknown_alternatives",
                        format!("{what} is an alternative of {id:?}, which does not exist"),
                    );
                    None
                }
            },
            None => None,
        };
        let published = shift.published.as_ref().and_then(|id| {
            let found = b.employee_ids.get(id).map(|&e| e as u32);
            if found.is_none() {
                b.error(
                    "unknown_employee",
                    format!("{what} was published to employee {id:?}, which does not exist"),
                );
            }
            found
        });
        let cost_group = shift.cost_group.as_ref().map(|g| b.intern(g));
        shifts.push(ShiftInfo {
            cost_group,
            alternative,
            published,
            location,
            location_id,
            start,
            end,
            day,
            last_day,
            minutes,
            breaks,
            logged: shift.logged_minutes.unwrap_or(minutes).max(0),
            weekend_minutes,
            tags,
            required_skills: b.needs(&shift.required_skills),
            any_required_skill: shift.any_required_skill,
            preferred_skills: b.needs(&shift.preferred_skills),
            risks: b.tag_set(&shift.risks),
            optional: shift.optional,
            importance: shift.importance.max(0),
            preferred_employees,
            unwanted_employees,
            barred_employees,
            in_horizon,
        });
    }

    let mut employees = employees;
    let mut teams = Vec::new();
    for (t, team) in plan.teams.iter().enumerate() {
        let what = format!("team {}", team.id);
        let leads = b.employee_list(&team.leads, || what.clone());
        let mut everyone = b.employee_list(&team.members, || what.clone());
        everyone.extend(&leads);
        everyone.sort_unstable();
        everyone.dedup();
        let tags = match &team.tag_type {
            Some(kind) => {
                let tags = b.tags_of_types(std::slice::from_ref(kind), &what);
                if tags.is_empty() {
                    continue;
                }
                tags
            }
            None => {
                if team.max_shifts_per_day.is_some() {
                    b.error(
                        "team_needs_tag_type",
                        format!("{what} limits shifts per day, which needs a tag type"),
                    );
                }
                Vec::new()
            }
        };
        for &e in &everyone {
            employees[e as usize].teams.push(t as u32);
        }
        teams.push(crate::derived::TeamInfo {
            leads,
            everyone,
            tags,
            max_shifts_per_day: team.max_shifts_per_day,
        });
    }

    for (s, info) in shifts.iter().enumerate() {
        if let Some(e) = info.published {
            employees[e as usize].published_shifts.push(s as u32);
        }
    }
    let rules = compile_schedule_rules(&mut b, &calendar, &plan.schedule_rules, &shifts);

    let derived = Derived {
        windows: rules.windows,
        shift_windows: rules.shift_windows,
        concurrency: rules.concurrency,
        slots: rules.slots,
        slots_by_day: rules.slots_by_day,
        alternatives: plan
            .alternatives
            .iter()
            .map(|a| (a.count, a.at_most))
            .collect(),
        replanning: plan.shifts.iter().any(|s| s.published.is_some()),
        shares,
        teams,
        origin: origin_day,
        horizon_start: h_start,
        horizon_end: h_end,
        days,
        week_start: plan.schedule.week_start.number(),
        weekend_mask,
        weekend_first,
        names: std::mem::take(&mut b.name_list),
        shifts,
        employees,
    };
    let ctx = Ctx(Arc::new(derived));
    for employee in &mut plan.employees {
        employee.ctx = ctx.clone();
    }
    for shift in &mut plan.shifts {
        shift.ctx = ctx.clone();
    }
    plan.days = (0..ctx.days.max(0) as usize)
        .map(|index| Day {
            index,
            ctx: ctx.clone(),
        })
        .collect();
    plan.windows = (0..ctx.windows.len())
        .map(|index| Window {
            index,
            ctx: ctx.clone(),
        })
        .collect();
    plan.choices = (0..ctx.alternatives.len())
        .map(|index| Choice {
            index,
            ctx: ctx.clone(),
        })
        .collect();
    plan.wholes = vec![Whole {
        index: 0,
        ctx: ctx.clone(),
    }];
    b.issues
}
