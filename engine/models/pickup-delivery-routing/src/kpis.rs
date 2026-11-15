//! Key performance indicators of a route plan.

use solverforge_workbench_host::Kpi;

use crate::derived::Kind;
use crate::domain::Plan;
use crate::route::{placement, routes};

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

pub fn kpis(plan: &Plan) -> Vec<Kpi> {
    let facts = &plan.facts;
    if facts.shifts.is_empty() {
        return Vec::new();
    }
    let routes = routes(plan);
    let at = placement(plan);
    let travel: i64 = routes.iter().map(|r| r.travel).sum();
    let distance: i64 = routes.iter().map(|r| r.distance).sum();
    let active = routes.iter().filter(|r| r.active).count();
    let overtime: i64 = routes
        .iter()
        .zip(&facts.shifts)
        .filter(|(r, _)| r.active)
        .map(|(r, s)| s.max_soft_end.map_or(0, |m| (r.end_arrival - m).max(0)))
        .sum();
    let mut stops = 0usize;
    let mut planned_stops = 0usize;
    let mut waiting = 0i64;
    let (mut with_sla, mut in_sla) = (0usize, 0usize);
    for (v, fact) in facts.visits.iter().enumerate() {
        if !matches!(fact.kind, Kind::Stop { .. }) {
            continue;
        }
        stops += 1;
        let Some((s, position)) = at[v] else {
            if fact.sla.is_some() {
                with_sla += 1;
            }
            continue;
        };
        planned_stops += 1;
        let visit = &routes[s].visits[position];
        waiting += visit.wait;
        if let Some(sla) = fact.sla {
            with_sla += 1;
            in_sla += usize::from(visit.end <= sla);
        }
    }
    let whole = |j: &crate::derived::JobFact| j.stops.iter().all(|&v| at[v].is_some());
    let planned_jobs = facts.jobs.iter().filter(|j| whole(j)).count();
    let mandatory_planned = facts
        .jobs
        .iter()
        .filter(|j| j.mandatory && whole(j))
        .count();
    let optional_planned = planned_jobs - mandatory_planned;
    // A job is pooled when another job is on board at some point of its ride.
    let mut pooled = 0usize;
    for job in &facts.jobs {
        let Some(ride) = span(&at, &job.stops) else {
            continue;
        };
        let others = facts.jobs.iter().filter(|o| o.id != job.id).any(|o| {
            span(&at, &o.stops)
                .is_some_and(|other| other.0 == ride.0 && other.1 < ride.2 && ride.1 < other.2)
        });
        pooled += usize::from(others);
    }
    let percent = |part: usize, whole: usize| {
        if whole == 0 {
            100.0
        } else {
            round(part as f64 * 100.0 / whole as f64)
        }
    };
    vec![
        kpi(
            "travel_time",
            "Travel time",
            round(travel as f64 / 3600.0),
            "h",
        ),
        kpi(
            "travel_distance",
            "Travel distance",
            round(distance as f64 / 1000.0),
            "km",
        ),
        kpi(
            "travel_per_stop",
            "Travel per stop",
            if planned_stops == 0 {
                0.0
            } else {
                round(travel as f64 / 60.0 / planned_stops as f64)
            },
            "min",
        ),
        kpi("assigned_jobs", "Jobs planned", planned_jobs as f64, "jobs"),
        kpi(
            "unassigned_jobs",
            "Jobs unplanned",
            (facts.jobs.len() - planned_jobs) as f64,
            "jobs",
        ),
        kpi(
            "mandatory_jobs",
            "Mandatory jobs planned",
            mandatory_planned as f64,
            "jobs",
        ),
        kpi(
            "optional_jobs",
            "Optional jobs planned",
            optional_planned as f64,
            "jobs",
        ),
        kpi(
            "assigned_stops",
            "Stops planned",
            planned_stops as f64,
            "stops",
        ),
        kpi(
            "unassigned_stops",
            "Stops unplanned",
            (stops - planned_stops) as f64,
            "stops",
        ),
        kpi("active_drivers", "Shifts driving", active as f64, "shifts"),
        kpi("overtime", "Overtime", round(overtime as f64 / 3600.0), "h"),
        kpi(
            "waiting",
            "Waiting time",
            round(waiting as f64 / 3600.0),
            "h",
        ),
        kpi(
            "stops_in_sla",
            "Stops within SLA",
            percent(in_sla, with_sla),
            "%",
        ),
        kpi(
            "pooled_jobs",
            "Jobs pooled",
            percent(pooled, facts.jobs.len()),
            "%",
        ),
    ]
}

/// A whole job's shift and its first and last position on it.
fn span(at: &[Option<(usize, usize)>], stops: &[usize]) -> Option<(usize, usize, usize)> {
    let mut shift = None;
    let (mut first, mut last) = (usize::MAX, 0);
    for &v in stops {
        let (s, p) = at[v]?;
        if shift.is_some_and(|x| x != s) {
            return None;
        }
        shift = Some(s);
        first = first.min(p);
        last = last.max(p);
    }
    shift.map(|s| (s, first, last))
}
