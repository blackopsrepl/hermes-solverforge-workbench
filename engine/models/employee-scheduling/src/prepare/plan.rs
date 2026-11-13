//! `prepare`: validate a dataset and derive its fact tables.

use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::Builder;
use crate::derived::{Ctx, Derived, EmployeeInfo, ShiftInfo};
use crate::domain::Plan;
use crate::time::{parse_date_time, DAY};

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
            EmployeeInfo {
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

    let h_start = horizon_start - b.origin_minutes;
    let h_end = horizon_end - b.origin_minutes;
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
        shifts.push(ShiftInfo {
            start,
            end,
            day,
            last_day,
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

    let mut weekend_mask = 0u8;
    for day in &plan.schedule.weekend {
        weekend_mask |= 1 << day.number();
    }

    let derived = Derived {
        origin: origin_day,
        horizon_start: h_start,
        horizon_end: h_end,
        days: (latest - b.origin_minutes + DAY - 1).div_euclid(DAY),
        week_start: plan.schedule.week_start.number(),
        weekend_mask,
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
    b.issues
}
