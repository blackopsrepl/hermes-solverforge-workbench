//! `prepare`: check a dataset, resolve its ids and derive what scoring reads.

use std::collections::HashMap;
use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::checks::Checks;
use super::tasks::read_tasks;
use crate::derived::{BayInfo, CrewInfo, Ctx, Derived, PartInfo, RuleInfo, UnitInfo};
use crate::domain::{BaySlot, Day, Plan};
use crate::time::parse_date;

/// Most days a horizon spans: about a quarter.
pub const MAX_HORIZON_DAYS: i64 = 92;
/// Most days one task takes.
pub const MAX_TASK_DAYS: i64 = 42;
/// Most crews one task draws on.
pub const MAX_LABOUR: usize = 4;
/// Most kinds of part one task consumes.
pub const MAX_PARTS: usize = 4;

/// Checks the dataset, resolves ids to indexes, derives the facts scoring
/// reads and the slots each task may take. Any error leaves the plan
/// unprepared.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    let mut checks = Checks {
        issues: Vec::new(),
        origin: 0,
        days: plan.horizon.days,
    };
    match parse_date(&plan.horizon.start) {
        Some(origin) => checks.origin = origin,
        None => checks.error(
            "invalid_horizon",
            format!(
                "the horizon start is not a date YYYY-MM-DD: {:?}",
                plan.horizon.start
            ),
        ),
    }
    if !(1..=MAX_HORIZON_DAYS).contains(&plan.horizon.days) {
        checks.error(
            "invalid_horizon",
            format!(
                "the horizon must span 1 to {MAX_HORIZON_DAYS} days, not {}",
                plan.horizon.days
            ),
        );
    }
    if checks.has_errors() {
        return checks.issues;
    }
    if plan.units.is_empty() {
        checks.error("no_units", "the dataset has no units".into());
    }
    if plan.bays.is_empty() {
        checks.error("no_bays", "the dataset has no bays".into());
    }
    checks.unique("unit", plan.units.iter().map(|u| u.id.as_str()));
    checks.unique("bay", plan.bays.iter().map(|b| b.id.as_str()));
    checks.unique("crew", plan.crews.iter().map(|c| c.id.as_str()));
    checks.unique("part", plan.parts.iter().map(|p| p.id.as_str()));
    checks.unique(
        "readiness rule",
        plan.readiness.iter().map(|r| r.id.as_str()),
    );
    checks.unique("task", plan.tasks.iter().map(|t| t.id.as_str()));
    let params = plan.parameters.clone();
    for (value, name) in [
        (params.overtime_cost, "overtimeCost"),
        (params.deferral_cost, "deferralCost"),
        (params.target_shortfall_cost, "targetShortfallCost"),
        (params.change_cost, "changeCost"),
        (params.load_level_cost, "loadLevelCost"),
        (params.induction_cost, "inductionCost"),
        (params.return_to_service_days, "returnToServiceDays"),
    ] {
        checks.not_negative(value, || format!("parameter {name}"));
    }

    let mut fleets: Vec<String> = Vec::new();
    let units = read_units(plan, &mut checks, &mut fleets);
    let bays = read_bays(plan, &mut checks, &fleets);
    let parts = read_parts(plan, &mut checks);
    let rules = read_rules(plan, &mut checks, &fleets, &units);
    let mut crews = read_crews(plan, &mut checks);
    let tasks = read_tasks(plan, &mut checks);
    if checks.has_errors() {
        return checks.issues;
    }
    let horizon = plan.horizon.days;
    for (crew, info) in crews.iter_mut().enumerate() {
        let work: i64 = tasks
            .iter()
            .flat_map(|task| {
                task.labour
                    .iter()
                    .filter(move |(c, _)| *c == crew)
                    .map(move |(_, n)| n * task.days)
            })
            .sum();
        info.level = (work + horizon - 1) / horizon;
    }
    let derived = Derived {
        origin: checks.origin,
        days: horizon,
        params,
        units,
        bays,
        crews,
        parts,
        rules,
        tasks,
    };
    let ctx = Ctx(Arc::new(derived));
    plan.slots = (0..plan.bays.len() * horizon as usize)
        .map(|index| BaySlot {
            index,
            ctx: ctx.clone(),
        })
        .collect();
    plan.days = (0..horizon as usize)
        .map(|index| Day {
            index,
            ctx: ctx.clone(),
        })
        .collect();
    let bay_index: HashMap<&str, usize> = plan
        .bays
        .iter()
        .enumerate()
        .map(|(i, bay)| (bay.id.as_str(), i))
        .collect();
    for (index, task) in plan.tasks.iter_mut().enumerate() {
        task.index = index;
        task.ctx = ctx.clone();
        task.slot_idx = match (&task.bay, &task.start) {
            (Some(bay), Some(start)) => {
                let day = parse_date(start).map(|d| d - ctx.origin);
                match (bay_index.get(bay.as_str()), day) {
                    (Some(&bay), Some(day)) => Some(ctx.slot(bay, day)),
                    _ => None,
                }
            }
            _ => None,
        };
        task.candidates = super::candidates(&ctx, index);
    }
    checks.issues
}

fn read_units(plan: &Plan, checks: &mut Checks, fleets: &mut Vec<String>) -> Vec<UnitInfo> {
    let mut out = Vec::with_capacity(plan.units.len());
    for unit in &plan.units {
        let fleet = match fleets.iter().position(|f| *f == unit.fleet) {
            Some(index) => index,
            None => {
                fleets.push(unit.fleet.clone());
                fleets.len() - 1
            }
        };
        if unit.priority < 1 {
            checks.error(
                "invalid_priority",
                format!("unit {} needs a priority of 1 or more", unit.id),
            );
        }
        checks.not_negative(unit.downtime_cost, || {
            format!("unit {} downtime cost", unit.id)
        });
        let available_from = unit
            .available_from
            .as_deref()
            .and_then(|date| checks.day(date, || format!("unit {} availableFrom", unit.id)))
            .unwrap_or(0)
            .max(0);
        if let Some(usage) = &unit.usage {
            if !usage.current.is_finite() || !usage.per_day.is_finite() || usage.per_day <= 0.0 {
                checks.error(
                    "invalid_usage",
                    format!(
                        "unit {} needs a finite usage counter that grows each day",
                        unit.id
                    ),
                );
            }
        }
        let commitments = unit
            .commitments
            .iter()
            .filter_map(|span| checks.span(span, &format!("unit {} commitment", unit.id)))
            .collect();
        out.push(UnitInfo {
            fleet,
            priority: unit.priority,
            downtime_cost: unit.downtime_cost,
            available_from,
            commitments,
        });
    }
    out
}

fn read_bays(plan: &Plan, checks: &mut Checks, fleets: &[String]) -> Vec<BayInfo> {
    let mut out = Vec::with_capacity(plan.bays.len());
    for bay in &plan.bays {
        if bay.capacity < 1 {
            checks.error(
                "invalid_capacity",
                format!("bay {} must hold at least one unit", bay.id),
            );
        }
        checks.not_negative(bay.cost_per_day, || format!("bay {} cost per day", bay.id));
        for fleet in &bay.fleets {
            if !fleets.contains(fleet) {
                checks.warning(
                    "unknown_fleet",
                    format!(
                        "bay {} holds fleet {fleet:?}, which no unit belongs to",
                        bay.id
                    ),
                );
            }
        }
        let closures = bay
            .closures
            .iter()
            .filter_map(|span| checks.span(span, &format!("bay {} closure", bay.id)))
            .collect();
        out.push(BayInfo {
            capacity: bay.capacity,
            cost_per_day: bay.cost_per_day,
            closures,
        });
    }
    out
}

fn read_crews(plan: &Plan, checks: &mut Checks) -> Vec<CrewInfo> {
    let days = plan.horizon.days;
    let mut out = Vec::with_capacity(plan.crews.len());
    for crew in &plan.crews {
        checks.not_negative(crew.technicians, || format!("crew {} technicians", crew.id));
        checks.not_negative(crew.overtime, || format!("crew {} overtime", crew.id));
        let mut regular = vec![crew.technicians; days as usize];
        for change in &crew.changes {
            checks.not_negative(change.technicians, || {
                format!("crew {} change technicians", crew.id)
            });
            let span = crate::wire::Span {
                from: change.from.clone(),
                to: change.to.clone(),
                label: None,
            };
            if let Some((from, to)) = checks.span(&span, &format!("crew {} change", crew.id)) {
                for day in from.max(0)..to.min(days) {
                    regular[day as usize] = change.technicians;
                }
            }
        }
        out.push(CrewInfo {
            regular,
            overtime: crew.overtime,
            level: 0,
        });
    }
    out
}

fn read_parts(plan: &Plan, checks: &mut Checks) -> Vec<PartInfo> {
    let mut out = Vec::with_capacity(plan.parts.len());
    for part in &plan.parts {
        checks.not_negative(part.on_hand, || format!("part {} on hand", part.id));
        let mut deliveries = Vec::new();
        for delivery in &part.deliveries {
            if delivery.quantity <= 0 {
                checks.error(
                    "invalid_quantity",
                    format!("a delivery of part {} must bring at least one", part.id),
                );
            }
            if let Some(day) = checks.day(&delivery.date, || format!("part {} delivery", part.id)) {
                deliveries.push((day.max(0), delivery.quantity));
            }
        }
        deliveries.sort_unstable();
        out.push(PartInfo {
            on_hand: part.on_hand,
            deliveries,
        });
    }
    out
}

fn read_rules(
    plan: &Plan,
    checks: &mut Checks,
    fleets: &[String],
    units: &[UnitInfo],
) -> Vec<RuleInfo> {
    let days = plan.horizon.days;
    let mut out = Vec::with_capacity(plan.readiness.len());
    for rule in &plan.readiness {
        let fleet = match &rule.fleet {
            Some(name) => match fleets.iter().position(|f| f == name) {
                Some(index) => Some(index),
                None => {
                    checks.error(
                        "unknown_fleet",
                        format!(
                            "readiness rule {} counts fleet {name:?}, which no unit belongs to",
                            rule.id
                        ),
                    );
                    continue;
                }
            },
            None => None,
        };
        checks.not_negative(rule.minimum, || {
            format!("readiness rule {} minimum", rule.id)
        });
        if rule.target < rule.minimum {
            checks.error(
                "invalid_readiness",
                format!(
                    "readiness rule {} targets {} ready units, below its minimum {}",
                    rule.id, rule.target, rule.minimum
                ),
            );
        }
        let first = rule
            .from
            .as_deref()
            .and_then(|d| checks.day(d, || format!("readiness rule {} from", rule.id)))
            .unwrap_or(0)
            .max(0);
        let end = rule
            .to
            .as_deref()
            .and_then(|d| checks.day(d, || format!("readiness rule {} to", rule.id)))
            .map_or(days, |to| to + 1)
            .min(days);
        let scope = units
            .iter()
            .filter(|unit| fleet.is_none_or(|f| f == unit.fleet))
            .count() as i64;
        if rule.target > scope {
            checks.warning(
                "readiness_unreachable",
                format!(
                    "readiness rule {} asks for {} ready units of the {scope} it counts",
                    rule.id, rule.target
                ),
            );
        }
        out.push(RuleInfo {
            fleet,
            minimum: rule.minimum,
            target: rule.target,
            first,
            end,
            scope,
        });
    }
    out
}
