//! Key performance indicators of a plan.

use solverforge_workbench_host::Kpi;

use crate::domain::Plan;
use crate::schedule::{furnace_changeovers, loads};

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
        0.0
    } else {
        round(100.0 * part / whole)
    }
}

pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let Some(d) = plan.work_orders.first().map(|o| o.ctx.clone()) else {
        return vec![kpi("orders", "Work orders", 0.0, "orders")];
    };
    let orders = plan.work_orders.len() as f64;
    let scheduled: Vec<_> = plan
        .work_orders
        .iter()
        .filter(|o| o.slot.is_some())
        .collect();
    let mut late = 0;
    let mut late_minutes = 0;
    for order in &scheduled {
        let p = order.placed().expect("scheduled");
        let over = p.end - d.orders[order.index].due;
        if over > 0 {
            late += 1;
            late_minutes += over;
        }
    }
    let loads = loads(plan);
    let busy: i64 = loads.iter().map(|l| l.end - l.start).sum();
    let fill: f64 = loads
        .iter()
        .map(|l| l.kg as f64 / d.furnaces[l.furnace].max_load_kg.max(1) as f64)
        .sum();
    let shared = loads
        .iter()
        .filter(|l| l.orders.len() > 1)
        .map(|l| l.orders.len())
        .sum::<usize>();
    let changeover: i64 = (0..plan.furnaces.len())
        .flat_map(|f| furnace_changeovers(plan, f))
        .map(|c| c.required)
        .sum();
    let energy: i64 = loads
        .iter()
        .map(|l| d.energy_cents(l.furnace, l.start, l.end))
        .sum();
    let kwh: f64 = loads
        .iter()
        .map(|l| d.furnaces[l.furnace].power_kw as f64 * (l.end - l.start) as f64 / 60.0)
        .sum();
    let peak_kwh: f64 = loads
        .iter()
        .map(|l| {
            d.furnaces[l.furnace].power_kw as f64 * d.peak_minutes(l.start, l.end) as f64 / 60.0
        })
        .sum();
    let shifts = plan.roster.iter().filter(|r| r.works_in_schedule()).count();
    let mut per_week = std::collections::HashMap::new();
    for row in plan.roster.iter().filter(|r| r.works_in_schedule()) {
        *per_week
            .entry((row.operator_idx(), row.day_index() / 7))
            .or_insert(0i64) += 1;
    }
    let overtime: i64 = per_week
        .values()
        .map(|n| (n - d.s.target_shifts_per_week).max(0))
        .sum();
    let open_tasks: usize = scheduled
        .iter()
        .map(|o| o.tasks().filter(|t| o.op(*t).is_none()).count())
        .sum();
    vec![
        kpi("orders", "Work orders", orders, "orders"),
        kpi(
            "scheduled",
            "Orders scheduled",
            scheduled.len() as f64,
            "orders",
        ),
        kpi(
            "unscheduled",
            "Orders not scheduled",
            orders - scheduled.len() as f64,
            "orders",
        ),
        kpi(
            "on_time",
            "Scheduled orders on time",
            percent((scheduled.len() - late) as f64, scheduled.len() as f64),
            "%",
        ),
        kpi("late_orders", "Late orders", late as f64, "orders"),
        kpi(
            "late_hours",
            "Total lateness",
            round(late_minutes as f64 / 60.0),
            "h",
        ),
        kpi("loads", "Furnace loads", loads.len() as f64, "loads"),
        kpi(
            "shared_orders",
            "Orders in shared loads",
            shared as f64,
            "orders",
        ),
        kpi(
            "fill",
            "Average load fill",
            percent(fill, loads.len() as f64),
            "%",
        ),
        kpi(
            "utilisation",
            "Furnace utilisation",
            percent(busy as f64, (d.horizon * plan.furnaces.len() as i64) as f64),
            "%",
        ),
        kpi(
            "changeover_hours",
            "Changeover time",
            round(changeover as f64 / 60.0),
            "h",
        ),
        kpi(
            "energy_cost",
            "Energy cost",
            round(energy as f64 / 100.0),
            "€",
        ),
        kpi("energy_kwh", "Energy used", kwh.round(), "kWh"),
        kpi(
            "peak_share",
            "Energy at peak tariff",
            percent(peak_kwh, kwh),
            "%",
        ),
        kpi("shifts", "Operator shifts", shifts as f64, "shifts"),
        kpi(
            "overtime_shifts",
            "Shifts above target",
            overtime as f64,
            "shifts",
        ),
        kpi(
            "open_tasks",
            "Tasks without an operator",
            open_tasks as f64,
            "tasks",
        ),
    ]
}
