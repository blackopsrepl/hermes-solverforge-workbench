//! Key performance indicators of a plan.

use solverforge_workbench_host::Kpi;

use crate::derived::{Derived, GlobalKind, Measure, ShiftInfo};
use crate::domain::Plan;

fn kpi(id: &'static str, label: &'static str, value: f64, unit: &'static str) -> Kpi {
    Kpi {
        id,
        label,
        value,
        unit,
    }
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn percent(part: f64, whole: f64) -> f64 {
    if whole <= 0.0 {
        100.0
    } else {
        round(100.0 * part / whole)
    }
}

/// Each employee's assigned in-horizon shifts, sorted by start.
fn rosters<'a>(plan: &Plan, d: &'a Derived) -> Vec<Vec<(usize, &'a ShiftInfo)>> {
    let mut out = vec![Vec::new(); d.employees.len()];
    for shift in &plan.shifts {
        if let Some(e) = shift.employee_idx {
            let info = &d.shifts[shift.index];
            if info.in_horizon {
                out[e].push((shift.index, info));
            }
        }
    }
    for roster in &mut out {
        roster.sort_by_key(|(s, info)| (info.start, *s));
    }
    out
}

/// Cents of an employee's contract pay and minutes beyond its base, by the
/// first contract cost rule; `None` without one.
fn contract_pay(d: &Derived, e: usize, roster: &[(usize, &ShiftInfo)]) -> Option<(i64, i64)> {
    let (mut cents, mut overtime) = (0, 0);
    let rule = d.employees[e]
        .window_rules
        .iter()
        .find(|rule| rule.measure == Measure::Cost)?;
    if let Some(tiers) = &rule.cost {
        for &(start, end) in &rule.windows {
            let minutes: i64 = roster
                .iter()
                .filter(|(_, i)| i.start >= start && i.start < end && rule.shifts.accepts(&i.tags))
                .map(|(_, i)| i.minutes)
                .sum();
            cents += tiers.cents(minutes);
            overtime += tiers.overtime_minutes(minutes);
        }
    }
    Some((cents, overtime))
}

/// Cents of the shifts of employees without contract pay, each priced once
/// by the first schedule-wide cost rule with a matching rate.
fn schedule_cost(plan: &Plan, d: &Derived, paid: &[bool]) -> i64 {
    let mut cents = 0;
    for shift in &plan.shifts {
        let Some(e) = shift.employee_idx.filter(|&e| !paid[e]) else {
            continue;
        };
        let info = &d.shifts[shift.index];
        if !info.in_horizon {
            continue;
        }
        for &w in &d.shift_windows[shift.index] {
            let rule = &d.windows[w as usize];
            if !matches!(rule.kind, GlobalKind::Cost | GlobalKind::CostTotal)
                || !rule
                    .employees
                    .as_ref()
                    .is_none_or(|f| f.accepts(&d.employees[e].tags))
            {
                continue;
            }
            let group = d.employees[e].cost_group;
            if let Some(rate) = rule.rates.iter().find(|rate| {
                rate.employee_group.is_none_or(|g| Some(g) == group)
                    && rate.shift_group.is_none_or(|g| Some(g) == info.cost_group)
            }) {
                cents += (info.minutes * rate.cents + 30) / 60;
                break;
            }
        }
    }
    cents
}

/// Share of demand minimums met, over all slots.
fn demand_coverage(plan: &Plan, d: &Derived) -> f64 {
    let (mut met, mut wanted) = (0, 0);
    for slot in &d.slots {
        let Some(min) = slot.min else {
            continue;
        };
        let staff = plan
            .shifts
            .iter()
            .filter(|shift| {
                let info = &d.shifts[shift.index];
                shift.employee_idx.is_some_and(|e| {
                    slot.employees
                        .as_ref()
                        .is_none_or(|f| f.accepts(&d.employees[e].tags))
                }) && info.start <= slot.start
                    && info.end >= slot.end
                    && slot.shifts.accepts(&info.tags)
            })
            .count() as i64;
        met += staff.min(min);
        wanted += min;
    }
    percent(met as f64, wanted as f64)
}

/// Jain's index of hours worked, as a percentage: 100 when everyone in a
/// fairness rule (everyone, without rules) works the same.
fn fairness(d: &Derived, hours: &[f64]) -> f64 {
    let ruled: Vec<f64> = (0..d.employees.len())
        .filter(|&e| !d.employees[e].fairness.is_empty())
        .map(|e| hours[e])
        .collect();
    let values = if ruled.is_empty() {
        hours.to_vec()
    } else {
        ruled
    };
    let sum: f64 = values.iter().sum();
    let squares: f64 = values.iter().map(|v| v * v).sum();
    if squares == 0.0 {
        return 100.0;
    }
    round(100.0 * sum * sum / (values.len() as f64 * squares))
}

/// Indicators of an assigned (and prepared) plan.
pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let Some(ctx) = plan
        .shifts
        .first()
        .map(|shift| shift.ctx.clone())
        .filter(|ctx| ctx.shifts.len() == plan.shifts.len())
    else {
        return vec![kpi("assigned_shifts", "Assigned shifts", 0.0, "shifts")];
    };
    let d: &Derived = &ctx;
    let in_horizon: Vec<_> = plan
        .shifts
        .iter()
        .filter(|shift| d.shifts[shift.index].in_horizon)
        .collect();
    let assigned = |optional: bool| {
        in_horizon
            .iter()
            .filter(|s| s.employee_idx.is_some() && d.shifts[s.index].optional == optional)
            .count() as f64
    };
    let (mandatory, optional) = (assigned(false), assigned(true));
    // Open alternatives are a choice, not a gap.
    let unassigned = in_horizon
        .iter()
        .filter(|s| s.employee_idx.is_none() && d.shifts[s.index].alternative.is_none())
        .count() as f64;

    let rosters = rosters(plan, d);
    let activated = rosters.iter().filter(|r| !r.is_empty()).count() as f64;
    let hours: Vec<f64> = rosters
        .iter()
        .map(|r| r.iter().map(|(_, i)| i.minutes).sum::<i64>() as f64 / 60.0)
        .collect();
    let total_hours: f64 = hours.iter().sum();
    let (mut pay, mut overtime) = (0, 0);
    let mut paid = vec![false; rosters.len()];
    let (mut liked, mut disliked, mut km) = (0, 0, 0);
    for (e, roster) in rosters.iter().enumerate() {
        if let Some((cents, minutes)) = contract_pay(d, e, roster) {
            paid[e] = true;
            pay += cents;
            overtime += minutes;
        }
        for &(s, _) in roster {
            let hit = |amount: i64| i64::from(amount > 0);
            liked += hit(d.preferred_employee(s, e)
                + d.preferred_minutes(s, e)
                + d.inside_preferred_shifts(s, e));
            disliked += hit(d.unwanted_employee(s, e) + d.unwanted_minutes(s, e));
            km += d.travel_km(s, e);
        }
    }

    let mut out = vec![
        kpi(
            "assigned_shifts",
            "Assigned shifts",
            mandatory + optional,
            "shifts",
        ),
        kpi(
            "unassigned_shifts",
            "Unassigned shifts",
            unassigned,
            "shifts",
        ),
        kpi(
            "assigned_mandatory_shifts",
            "Assigned mandatory shifts",
            mandatory,
            "shifts",
        ),
        kpi(
            "assigned_optional_shifts",
            "Assigned optional shifts",
            optional,
            "shifts",
        ),
        kpi(
            "activated_employees",
            "Employees with shifts",
            activated,
            "employees",
        ),
        kpi("total_hours", "Hours worked", round(total_hours), "h"),
        kpi(
            "average_hours",
            "Average hours per working employee",
            round(if activated > 0.0 {
                total_hours / activated
            } else {
                0.0
            }),
            "h",
        ),
        kpi(
            "overtime_hours",
            "Overtime hours",
            round(overtime as f64 / 60.0),
            "h",
        ),
        kpi(
            "total_cost",
            "Total cost",
            round((pay + schedule_cost(plan, d, &paid)) as f64 / 100.0),
            "currency",
        ),
        kpi("preferences_met", "Preferences met", liked as f64, "shifts"),
        kpi("dislikes_hit", "Dislikes hit", disliked as f64, "shifts"),
        kpi("travel_distance", "Travel distance", km as f64, "km"),
        kpi(
            "fairness",
            "Working-time fairness",
            fairness(d, &hours),
            "%",
        ),
    ];
    if !d.slots.is_empty() {
        out.push(kpi(
            "demand_coverage",
            "Demand covered",
            demand_coverage(plan, d),
            "%",
        ));
    }
    if d.replanning {
        let published: Vec<_> = in_horizon
            .iter()
            .filter_map(|s| d.shifts[s.index].published.map(|e| (s, e as usize)))
            .collect();
        let moved = published
            .iter()
            .filter(|(s, e)| s.employee_idx != Some(*e))
            .count();
        out.push(kpi(
            "disruption",
            "Published shifts moved",
            percent(moved as f64, published.len() as f64),
            "%",
        ));
    }
    out
}
