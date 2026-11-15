//! Key performance indicators of a plan.

use solverforge_workbench_host::Kpi;

use crate::derived::Derived;
use crate::domain::Plan;
use crate::status::Fleet;
use crate::time::overlap;

fn kpi(id: &'static str, label: &'static str, value: f64, unit: &'static str) -> Kpi {
    Kpi {
        id,
        label,
        value,
        unit,
    }
}

fn round(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn percent(part: f64, whole: f64) -> f64 {
    if whole <= 0.0 {
        100.0
    } else {
        round(100.0 * part / whole)
    }
}

/// Technicians each crew needs each day, `[crew][day]`.
fn crew_demand(plan: &Plan, d: &Derived) -> Vec<Vec<i64>> {
    let mut demand = vec![vec![0; d.days as usize]; d.crews.len()];
    for task in &plan.tasks {
        let Some(slot) = task.slot_idx else { continue };
        let (start, end) = d.occupied(task.index, slot);
        for &(crew, technicians) in &d.tasks[task.index].labour {
            for day in start.max(0)..end.min(d.days) {
                demand[crew][day as usize] += technicians;
            }
        }
    }
    demand
}

/// Parts missing when tasks need them, over every part.
fn parts_short(plan: &Plan, d: &Derived) -> i64 {
    let mut uses: Vec<Vec<(i64, usize, i64)>> = vec![Vec::new(); d.parts.len()];
    for task in &plan.tasks {
        let Some(slot) = task.slot_idx else { continue };
        for &(part, quantity) in &d.tasks[task.index].parts {
            uses[part].push((d.slot_day(slot), task.index, quantity));
        }
    }
    let mut short = 0;
    for (part, mut rows) in uses.into_iter().enumerate() {
        rows.sort_unstable();
        let mut used = 0;
        for (day, _, quantity) in rows {
            let take = quantity.min((d.parts[part].supplied_by(day) - used).max(0));
            used += take;
            short += quantity - take;
        }
    }
    short
}

pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let Some(fleet) = Fleet::of(plan) else {
        return Vec::new();
    };
    let d = fleet.d;
    let days = d.days as usize;
    let units = d.units.len() as f64;
    let daily: Vec<f64> = (0..days)
        .map(|day| percent(fleet.ready_count(day) as f64, units))
        .collect();
    let average = round(daily.iter().sum::<f64>() / days.max(1) as f64);
    let lowest = daily.iter().copied().fold(100.0, f64::min);
    let below_minimum = (0..days).filter(|&day| fleet.below_minimum(day)).count();
    let below_target = (0..days).filter(|&day| fleet.below_target(day)).count();

    let planned = plan.tasks.iter().filter(|t| t.slot_idx.is_some()).count();
    let open_mandatory = plan
        .tasks
        .iter()
        .filter(|t| t.slot_idx.is_none() && !d.tasks[t.index].deferrable)
        .count();
    let deferred = plan
        .tasks
        .iter()
        .filter(|t| t.slot_idx.is_none() && d.tasks[t.index].deferrable)
        .count();
    let overdue = plan
        .tasks
        .iter()
        .filter(|t| {
            t.slot_idx.is_some_and(|slot| {
                d.late_days(t.index, slot) > 0 || d.usage_days(t.index, slot) > 0
            })
        })
        .count();
    let moved = plan
        .tasks
        .iter()
        .filter(|t| d.moved(t.index, t.slot_idx))
        .count();

    let mut held_days = 0;
    let mut downtime = 0;
    for (day, held) in fleet.held.iter().enumerate() {
        for (unit, down) in held.iter().enumerate() {
            if *down && !d.away(unit, day as i64) {
                held_days += 1;
                downtime += d.units[unit].downtime_cost;
            }
        }
    }

    let occupied: i64 = plan
        .tasks
        .iter()
        .filter_map(|t| t.slot_idx.map(|slot| (t.index, slot)))
        .map(|(task, slot)| {
            let (start, end) = d.occupied(task, slot);
            overlap(start, end, 0, d.days)
        })
        .sum();
    let room: i64 = d
        .bays
        .iter()
        .map(|bay| {
            let closed: i64 = bay
                .closures
                .iter()
                .map(|&(from, to)| overlap(from, to, 0, d.days))
                .sum();
            bay.capacity * (d.days - closed).max(0)
        })
        .sum();

    let demand = crew_demand(plan, d);
    let overtime: i64 = demand
        .iter()
        .zip(&d.crews)
        .map(|(per_day, crew)| {
            per_day
                .iter()
                .zip(&crew.regular)
                .map(|(need, regular)| (need - regular).clamp(0, crew.overtime))
                .sum::<i64>()
        })
        .sum();
    let bay_cost: i64 = plan
        .tasks
        .iter()
        .filter_map(|t| t.slot_idx.map(|slot| d.bay_cost(t.index, slot)))
        .sum();

    vec![
        kpi("readiness_average", "Average daily readiness", average, "%"),
        kpi("readiness_lowest", "Lowest daily readiness", lowest, "%"),
        kpi(
            "days_below_minimum",
            "Days below a readiness minimum",
            below_minimum as f64,
            "days",
        ),
        kpi(
            "days_below_target",
            "Days below a readiness target",
            below_target as f64,
            "days",
        ),
        kpi("tasks_planned", "Tasks planned", planned as f64, "tasks"),
        kpi(
            "mandatory_unplanned",
            "Mandatory tasks unplanned",
            open_mandatory as f64,
            "tasks",
        ),
        kpi(
            "deferred",
            "Deferrable tasks deferred",
            deferred as f64,
            "tasks",
        ),
        kpi(
            "overdue",
            "Tasks done past a due limit",
            overdue as f64,
            "tasks",
        ),
        kpi(
            "downtime_days",
            "Unit-days out of service",
            held_days as f64,
            "unit-days",
        ),
        kpi(
            "bay_utilisation",
            "Bay utilisation",
            percent(occupied as f64, room as f64),
            "%",
        ),
        kpi(
            "overtime_days",
            "Overtime",
            overtime as f64,
            "technician-days",
        ),
        kpi(
            "parts_short",
            "Parts short",
            parts_short(plan, d) as f64,
            "parts",
        ),
        kpi(
            "maintenance_cost",
            "Bay and overtime cost",
            (bay_cost + overtime * d.params.overtime_cost) as f64,
            "currency",
        ),
        kpi(
            "downtime_cost",
            "Downtime cost",
            downtime as f64,
            "currency",
        ),
        kpi(
            "tasks_moved",
            "Published tasks moved",
            moved as f64,
            "tasks",
        ),
    ]
}
