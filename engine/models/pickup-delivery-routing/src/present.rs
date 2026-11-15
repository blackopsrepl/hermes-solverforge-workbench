//! The plan written back in the dataset's own format, with its routes.
//!
//! Each shift's `itinerary` lists its stops and breaks in driving order,
//! with arrival, service and departure times, travel and load, so a solved
//! plan is also a valid dataset to replan from.

use serde_json::{json, Value};

use crate::derived::Kind;
use crate::domain::Plan;
use crate::time::format_date_time;

pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    let facts = &plan.facts;
    if facts.shifts.is_empty() {
        return value;
    }
    let routes = crate::route::routes(plan);
    let at = crate::route::placement(plan);
    let mut s = 0;
    if let Some(drivers) = value.get_mut("drivers").and_then(Value::as_array_mut) {
        for driver in drivers {
            let Some(shifts) = driver.get_mut("shifts").and_then(Value::as_array_mut) else {
                continue;
            };
            for json in shifts {
                let shift = &plan.shifts[s];
                let route = &routes[s];
                let fact = &facts.shifts[s];
                let mut load = vec![0i64; facts.capacity_types.len()];
                let itinerary: Vec<Value> = shift
                    .visits
                    .iter()
                    .zip(&route.visits)
                    .map(|(&v, visit)| {
                        let vf = &facts.visits[v];
                        let mut item = json!({
                            "id": vf.id,
                            "kind": if matches!(vf.kind, Kind::Stop { .. }) { "STOP" } else { "BREAK" },
                            "arrivalTime": format_date_time(visit.arrival),
                            "startServiceTime": format_date_time(visit.start),
                            "departureTime": format_date_time(visit.end),
                            "travelSeconds": visit.travel,
                            "travelMeters": visit.distance,
                        });
                        if let Some((_, start)) = vf.pin {
                            item["pin"] = json!(true);
                            item["startServiceTime"] = json!(format_date_time(start));
                        }
                        if visit.unreachable {
                            item["unreachable"] = json!(true);
                        }
                        if matches!(vf.kind, Kind::Stop { .. }) && !load.is_empty() {
                            let rows: Vec<Value> = facts
                                .capacity_types
                                .iter()
                                .enumerate()
                                .map(|(k, kind)| {
                                    let previous = load[k];
                                    load[k] += vf.load[k];
                                    json!({"type": kind, "previous": previous, "current": load[k]})
                                })
                                .collect();
                            item["load"] = Value::Array(rows);
                        }
                        item
                    })
                    .collect();
                json["itinerary"] = Value::Array(itinerary);
                if route.active {
                    json["metrics"] = json!({
                        "startTime": format_date_time(route.start),
                        "endLocationArrivalTime": format_date_time(route.end_arrival),
                        "lastStopDepartureTime": format_date_time(route.last_departure),
                        "travelSeconds": route.travel,
                        "travelMeters": route.distance,
                        "overtimeSeconds": fact.max_soft_end.map_or(0, |m| (route.end_arrival - m).max(0)),
                    });
                    if route.unreachable_legs > 0 {
                        json["unreachable"] = json!(true);
                    }
                }
                s += 1;
            }
        }
    }
    let unplanned = |v: usize| at[v].is_none();
    value["unassignedJobs"] = json!(facts
        .jobs
        .iter()
        .filter(|j| j.stops.iter().any(|&v| unplanned(v)))
        .map(|j| j.id.clone())
        .collect::<Vec<_>>());
    value["unassignedStops"] = json!(facts
        .visits
        .iter()
        .enumerate()
        .filter(|(v, f)| matches!(f.kind, Kind::Stop { .. }) && unplanned(*v))
        .map(|(_, f)| f.id.clone())
        .collect::<Vec<_>>());
    value
}
