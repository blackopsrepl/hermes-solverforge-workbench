//! Key performance indicators of a route plan.

use solverforge_workbench_host::Kpi;

use crate::domain::Plan;
use crate::present::timed;
use crate::route::routes;

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
    let plan = timed(plan);
    let routes = routes(&plan);
    let travel: i64 = routes.iter().map(|r| r.travel).sum();
    let distance: i64 = routes.iter().map(|r| r.distance).sum();
    let overtime: i64 = routes.iter().map(|r| r.overtime).sum();
    let cost: i64 = routes.iter().filter(|r| r.active).map(|r| r.cost).sum();
    let waiting: i64 = routes.iter().map(|r| r.waiting).sum();
    let active_vehicles = {
        let mut seen: Vec<usize> = routes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.active)
            .map(|(s, _)| facts.shifts[s].vehicle)
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    };
    let (mut planned, mut mandatory, mut optional) = (0usize, 0usize, 0usize);
    let (mut mandatory_total, mut with_sla, mut in_sla) = (0usize, 0usize, 0usize);
    let (mut outside_preferred, mut profit, mut rated) = (0usize, 0i64, (0.0f64, 0usize));
    for (v, fact) in facts.visits[..facts.first_break].iter().enumerate() {
        mandatory_total += usize::from(fact.mandatory);
        let slot = plan.elements[v].planned_slot();
        if fact.sla.is_some() {
            with_sla += 1;
        }
        let Some(slot) = slot else { continue };
        planned += 1;
        if fact.mandatory {
            mandatory += 1;
        } else {
            optional += 1;
        }
        if fact.sla.is_some_and(|sla| slot.at.end <= sla) {
            in_sla += 1;
        }
        outside_preferred += usize::from(facts.fit(v, slot.shift).outside_preferred);
        profit += fact.profit;
        rated.0 += facts.vehicles[slot.vehicle].rating;
        rated.1 += 1;
    }
    let visits = facts.first_break;
    let percent = |part: usize, whole: usize| {
        if whole == 0 {
            100.0
        } else {
            round(part as f64 * 100.0 / whole as f64)
        }
    };
    // Fairness: one minus the spread of utilization across technicians.
    let mut worked = vec![0i64; facts.vehicles.len()];
    for (s, route) in routes.iter().enumerate() {
        if route.active {
            worked[facts.shifts[s].vehicle] += route.service + route.travel;
        }
    }
    let shares: Vec<f64> = facts
        .vehicles
        .iter()
        .zip(&worked)
        .filter(|(v, _)| v.capacity + v.historical_capacity > 0)
        .map(|(v, &w)| {
            (v.historical_utilized + w) as f64 / (v.historical_capacity + v.capacity) as f64
        })
        .collect();
    let fairness = if shares.is_empty() {
        100.0
    } else {
        let mean = shares.iter().sum::<f64>() / shares.len() as f64;
        let deviation =
            (shares.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / shares.len() as f64).sqrt();
        round((1.0 - deviation).max(0.0) * 100.0)
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
            "travel_per_visit",
            "Travel per visit",
            if planned == 0 {
                0.0
            } else {
                round(travel as f64 / 60.0 / planned as f64)
            },
            "min",
        ),
        kpi(
            "assigned_visits",
            "Visits planned",
            planned as f64,
            "visits",
        ),
        kpi(
            "unassigned_visits",
            "Visits unplanned",
            (visits - planned) as f64,
            "visits",
        ),
        kpi(
            "mandatory_visits",
            "Mandatory visits planned",
            mandatory as f64,
            "visits",
        ),
        kpi(
            "unassigned_mandatory",
            "Mandatory visits unplanned",
            (mandatory_total - mandatory) as f64,
            "visits",
        ),
        kpi(
            "optional_visits",
            "Optional visits planned",
            optional as f64,
            "visits",
        ),
        kpi(
            "active_vehicles",
            "Technicians working",
            active_vehicles as f64,
            "technicians",
        ),
        kpi("overtime", "Overtime", round(overtime as f64 / 3600.0), "h"),
        kpi(
            "waiting",
            "Waiting time",
            round(waiting as f64 / 3600.0),
            "h",
        ),
        kpi("fairness", "Working time fairness", fairness, "%"),
        kpi("cost", "Technician costs", round(cost as f64 / 100.0), "€"),
        kpi(
            "visits_in_sla",
            "Visits within SLA",
            percent(in_sla, with_sla),
            "%",
        ),
        kpi(
            "outside_preferred_area",
            "Visits outside preferred area",
            percent(outside_preferred, planned),
            "%",
        ),
        kpi(
            "average_rating",
            "Average technician rating",
            if rated.1 == 0 {
                0.0
            } else {
                round(rated.0 / rated.1 as f64)
            },
            "",
        ),
        kpi("profit", "Profit", round(profit as f64 / 100.0), "€"),
    ]
}
