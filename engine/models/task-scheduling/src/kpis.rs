//! Key performance indicators of a schedule.

use solverforge_workbench_host::Kpi;

use crate::domain::Plan;
use crate::schedule::Timing;

fn kpi(id: &'static str, label: &'static str, value: f64, unit: &'static str) -> Kpi {
    Kpi {
        id,
        label,
        value,
        unit,
    }
}

fn hours(minutes: i64) -> f64 {
    (minutes as f64 / 6.0).round() / 10.0
}

fn percent(part: i64, whole: i64) -> f64 {
    if whole <= 0 {
        0.0
    } else {
        (part as f64 * 1000.0 / whole as f64).round() / 10.0
    }
}

pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let facts = &*plan.facts;
    let timing = Timing::of(plan);
    let n = facts.tasks.len();
    let assigned = (0..n).filter(|&t| timing.assigned(t)).count();
    let schedule_end = (0..n)
        .filter(|&t| timing.assigned(t))
        .map(|t| timing.end[t])
        .max();
    let mut makespan = 0;
    let (mut machines, mut employees) = (0, 0);
    let (mut machine_work, mut machine_time) = (0, 0);
    let (mut employee_work, mut employee_time) = (0, 0);
    let (mut changeovers, mut changeover_minutes, mut idle) = (0, 0, 0);
    for (r, order) in timing.order.iter().enumerate() {
        let rf = &facts.resources[r];
        if let Some(last) = order.iter().map(|&t| timing.end[t]).max() {
            makespan = makespan.max(last - rf.start);
            if rf.machine {
                machines += 1;
            } else {
                employees += 1;
            }
        }
        let work: i64 = order.iter().map(|&t| facts.tasks[t].duration[r]).sum();
        let available = schedule_end.map_or(0, |end| {
            (end - rf.start - rf.calendar.unavailable_between(rf.start, end)).max(0)
        });
        if rf.machine {
            machine_work += work;
            machine_time += available;
        } else {
            employee_work += work;
            employee_time += available;
        }
        for &t in order {
            if let Some(changeover) = timing.changeover[t] {
                changeovers += 1;
                changeover_minutes += changeover.minutes;
            }
            idle += timing.idle[t];
        }
    }
    let (mut deviation, mut late) = (0, 0);
    for (t, tf) in facts.tasks.iter().enumerate() {
        if !timing.assigned(t) {
            continue;
        }
        if let Some(ideal) = tf.ideal_end {
            deviation += (timing.end[t] - ideal).abs();
        }
        let after = |limit: Option<i64>| limit.is_some_and(|limit| timing.end[t] > limit);
        if after(tf.due) || after(tf.max_end) {
            late += 1;
        }
    }
    vec![
        kpi(
            "unassigned_tasks",
            "Unassigned tasks",
            (n - assigned) as f64,
            "tasks",
        ),
        kpi("makespan", "Makespan", hours(makespan), "h"),
        kpi("assigned_tasks", "Assigned tasks", assigned as f64, "tasks"),
        kpi("late_tasks", "Late tasks", late as f64, "tasks"),
        kpi(
            "ideal_end_deviation",
            "Ideal end deviation",
            hours(deviation),
            "h",
        ),
        kpi(
            "active_machines",
            "Machines in use",
            machines as f64,
            "machines",
        ),
        kpi(
            "active_employees",
            "Employees in use",
            employees as f64,
            "employees",
        ),
        kpi(
            "machine_utilization",
            "Machine utilisation",
            percent(machine_work, machine_time),
            "%",
        ),
        kpi(
            "employee_utilization",
            "Employee utilisation",
            percent(employee_work, employee_time),
            "%",
        ),
        kpi(
            "changeovers",
            "Changeovers",
            changeovers as f64,
            "changeovers",
        ),
        kpi(
            "changeover_time",
            "Changeover time",
            hours(changeover_minutes),
            "h",
        ),
        kpi("idle_time", "Machine idle time", hours(idle), "h"),
    ]
}
