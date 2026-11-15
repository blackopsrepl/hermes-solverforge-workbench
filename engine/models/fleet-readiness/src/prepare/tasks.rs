//! Reading the tasks: ids, windows, due limits, crews, parts and precedence.

use std::collections::HashMap;

use super::checks::Checks;
use super::plan::{MAX_LABOUR, MAX_PARTS, MAX_TASK_DAYS};
use crate::derived::TaskInfo;
use crate::domain::{Plan, Task};

fn index_of<'a>(ids: impl Iterator<Item = &'a str>) -> HashMap<&'a str, usize> {
    ids.enumerate().map(|(i, id)| (id, i)).collect()
}

pub fn read_tasks(plan: &Plan, checks: &mut Checks) -> Vec<TaskInfo> {
    let unit_ids = index_of(plan.units.iter().map(|u| u.id.as_str()));
    let bay_ids = index_of(plan.bays.iter().map(|b| b.id.as_str()));
    let crew_ids = index_of(plan.crews.iter().map(|c| c.id.as_str()));
    let part_ids = index_of(plan.parts.iter().map(|p| p.id.as_str()));
    let task_ids = index_of(plan.tasks.iter().map(|t| t.id.as_str()));
    let classes: Vec<&str> = plan.bays.iter().map(|b| b.class.as_str()).collect();
    let mut out = Vec::with_capacity(plan.tasks.len());
    for task in &plan.tasks {
        let Some(&unit) = unit_ids.get(task.unit.as_str()) else {
            checks.error(
                "unknown_unit",
                format!("task {} is for unknown unit {:?}", task.id, task.unit),
            );
            continue;
        };
        if !(1..=MAX_TASK_DAYS).contains(&task.days) {
            checks.error(
                "invalid_length",
                format!(
                    "task {} must take 1 to {MAX_TASK_DAYS} days, not {}",
                    task.id, task.days
                ),
            );
        }
        let info = read_task(
            task,
            unit,
            checks,
            Lookups {
                plan,
                bay_ids: &bay_ids,
                crew_ids: &crew_ids,
                part_ids: &part_ids,
                task_ids: &task_ids,
                classes: &classes,
            },
        );
        out.push(info);
    }
    if !checks.has_errors() {
        check_cycles(plan, checks, &out);
    }
    out
}

struct Lookups<'a> {
    plan: &'a Plan,
    bay_ids: &'a HashMap<&'a str, usize>,
    crew_ids: &'a HashMap<&'a str, usize>,
    part_ids: &'a HashMap<&'a str, usize>,
    task_ids: &'a HashMap<&'a str, usize>,
    classes: &'a [&'a str],
}

fn read_task(task: &Task, unit: usize, checks: &mut Checks, at: Lookups<'_>) -> TaskInfo {
    let horizon = checks.days;
    let id = &task.id;
    let earliest = task
        .earliest_start
        .as_deref()
        .and_then(|d| checks.day(d, || format!("task {id} earliestStart")))
        .unwrap_or(0)
        .max(0);
    let due = task
        .due_date
        .as_deref()
        .and_then(|d| checks.day(d, || format!("task {id} dueDate")));
    let usage_day = match (task.due_usage, &at.plan.units[unit].usage) {
        (None, _) => None,
        (Some(limit), Some(usage)) if usage.per_day > 0.0 => {
            Some(((limit as f64 - usage.current) / usage.per_day).floor() as i64)
        }
        (Some(_), Some(_)) => None,
        (Some(_), None) => {
            checks.error(
                "usage_unknown",
                format!(
                    "task {id} is due at a usage limit but unit {} has no usage counter",
                    task.unit
                ),
            );
            None
        }
    };
    let grounded_from = match (due, usage_day) {
        (Some(d), Some(u)) => Some((d + 1).min(u + 1)),
        (Some(d), None) => Some(d + 1),
        (None, Some(u)) => Some(u + 1),
        (None, None) => None,
    };
    if grounded_from.is_some_and(|g| g <= 0) {
        checks.warning(
            "overdue_at_start",
            format!("task {id} is already overdue when the horizon starts; its unit is grounded until it is done"),
        );
    }
    if earliest >= horizon {
        checks.warning(
            "window_outside_horizon",
            format!("task {id} cannot start inside the horizon"),
        );
    }
    let return_to_service = task
        .return_to_service
        .unwrap_or(at.plan.parameters.return_to_service_days);
    checks.not_negative(return_to_service, || format!("task {id} returnToService"));

    let fleet = &at.plan.units[unit].fleet;
    for class in &task.bay_classes {
        if !at.classes.contains(&class.as_str()) {
            checks.warning(
                "unknown_bay_class",
                format!("task {id} asks for bay class {class:?}, which no bay has"),
            );
        }
    }
    let suitable: Vec<bool> = at
        .plan
        .bays
        .iter()
        .map(|bay| {
            (task.bay_classes.is_empty() || task.bay_classes.contains(&bay.class))
                && (bay.fleets.is_empty() || bay.fleets.contains(fleet))
        })
        .collect();
    if !suitable.iter().any(|s| *s) {
        checks.warning(
            "no_suitable_bay",
            format!("no bay can take task {id}; it stays unplanned"),
        );
    }

    if task.labour.len() > MAX_LABOUR {
        checks.error(
            "invalid_labour",
            format!("task {id} draws on more than {MAX_LABOUR} crews"),
        );
    }
    let mut labour: Vec<(usize, i64)> = Vec::new();
    for need in &task.labour {
        match at.crew_ids.get(need.crew.as_str()) {
            None => checks.error(
                "unknown_crew",
                format!("task {id} needs unknown crew {:?}", need.crew),
            ),
            Some(_) if need.technicians <= 0 => checks.error(
                "invalid_labour",
                format!(
                    "task {id} must need at least one technician of crew {}",
                    need.crew
                ),
            ),
            Some(&crew) if labour.iter().any(|(c, _)| *c == crew) => checks.error(
                "invalid_labour",
                format!("task {id} lists crew {} twice", need.crew),
            ),
            Some(&crew) => labour.push((crew, need.technicians)),
        }
    }
    if task.parts.len() > MAX_PARTS {
        checks.error(
            "invalid_parts",
            format!("task {id} consumes more than {MAX_PARTS} kinds of part"),
        );
    }
    let mut parts: Vec<(usize, i64)> = Vec::new();
    for need in &task.parts {
        match at.part_ids.get(need.part.as_str()) {
            None => checks.error(
                "unknown_part",
                format!("task {id} consumes unknown part {:?}", need.part),
            ),
            Some(_) if need.quantity <= 0 => checks.error(
                "invalid_parts",
                format!("task {id} must consume at least one part {}", need.part),
            ),
            Some(&part) if parts.iter().any(|(p, _)| *p == part) => checks.error(
                "invalid_parts",
                format!("task {id} lists part {} twice", need.part),
            ),
            Some(&part) => parts.push((part, need.quantity)),
        }
    }
    let mut preds = Vec::new();
    for before in &task.after {
        match at.task_ids.get(before.as_str()) {
            None => checks.error(
                "unknown_task",
                format!("task {id} follows unknown task {before:?}"),
            ),
            Some(_) if before == id => checks.error(
                "precedence_cycle",
                format!("task {id} cannot follow itself"),
            ),
            Some(&other) if at.plan.tasks[other].unit != task.unit => checks.error(
                "precedence_across_units",
                format!("task {id} follows task {before}, which is for another unit"),
            ),
            Some(&other) => preds.push(other),
        }
    }

    let published = task.published.as_ref().and_then(|p| {
        let bay = at.bay_ids.get(p.bay.as_str()).copied();
        let day = checks.day(&p.start, || format!("task {id} published start"));
        match (bay, day) {
            (None, _) => {
                checks.error(
                    "unknown_bay",
                    format!("task {id} was published to unknown bay {:?}", p.bay),
                );
                None
            }
            (Some(bay), Some(day)) if (0..horizon).contains(&day) => {
                Some(bay * horizon as usize + day as usize)
            }
            (Some(_), Some(_)) => {
                checks.warning(
                    "published_outside_horizon",
                    format!(
                        "task {id} was published to start outside the horizon; that is ignored"
                    ),
                );
                None
            }
            (Some(_), None) => None,
        }
    });
    check_assignment(task, checks, at.bay_ids);

    let days = task.days;
    TaskInfo {
        unit,
        days,
        earliest,
        finish_by: due.map(|d| (d + 1).max(earliest + days)),
        start_by: usage_day.map(|u| u.max(earliest)),
        grounded_from,
        buffer: return_to_service,
        suitable,
        labour,
        parts,
        preds,
        published,
        deferrable: task.deferrable,
    }
}

fn check_assignment(task: &Task, checks: &mut Checks, bay_ids: &HashMap<&str, usize>) {
    let id = &task.id;
    match (&task.bay, &task.start) {
        (Some(bay), Some(start)) => {
            if !bay_ids.contains_key(bay.as_str()) {
                checks.error(
                    "unknown_bay",
                    format!("task {id} is assigned to unknown bay {bay:?}"),
                );
            }
            if let Some(day) = checks.day(start, || format!("task {id} start")) {
                if !(0..checks.days).contains(&day) {
                    checks.error(
                        "start_outside_horizon",
                        format!("task {id} starts on {start}, outside the horizon"),
                    );
                }
            }
        }
        (None, None) => {
            if task.pinned {
                checks.error(
                    "pinned_unassigned",
                    format!("task {id} is pinned but has no bay and start"),
                );
            }
        }
        _ => checks.error(
            "partial_assignment",
            format!("task {id} needs both a bay and a start, or neither"),
        ),
    }
}

/// Reports precedence that loops back on itself.
fn check_cycles(plan: &Plan, checks: &mut Checks, tasks: &[TaskInfo]) {
    // 0 unvisited, 1 on the current path, 2 done.
    let mut state = vec![0u8; tasks.len()];
    for root in 0..tasks.len() {
        if state[root] != 0 {
            continue;
        }
        let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
        state[root] = 1;
        while let Some(&mut (task, ref mut next)) = stack.last_mut() {
            if let Some(&pred) = tasks[task].preds.get(*next) {
                *next += 1;
                match state[pred] {
                    0 => {
                        state[pred] = 1;
                        stack.push((pred, 0));
                    }
                    1 => {
                        checks.error(
                            "precedence_cycle",
                            format!(
                                "tasks {} and {} wait for each other",
                                plan.tasks[task].id, plan.tasks[pred].id
                            ),
                        );
                        return;
                    }
                    _ => {}
                }
            } else {
                state[task] = 2;
                stack.pop();
            }
        }
    }
}
