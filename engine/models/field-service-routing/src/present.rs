//! The plan written back in the dataset's own format, with its routes.
//!
//! Each shift's `itinerary` lists its visits and breaks in driving order,
//! with arrival, service and departure times and travel, so a solved plan
//! is also a valid dataset to replan from.

use serde_json::{json, Value};

use crate::domain::Plan;
use crate::time::format_date_time;

/// The plan with every shadow computed from its lists.
pub fn timed(plan: &Plan) -> Plan {
    use solverforge::PlanningSolution;
    let mut plan = plan.clone();
    if !plan.facts.shifts.is_empty() {
        plan.update_all_shadows();
    }
    plan
}

/// One shift's itinerary and metrics, written into its wire record.
fn write(plan: &Plan, s: usize, route: &crate::route::Route, json: &mut Value) {
    let facts = &plan.facts;
    let itinerary: Vec<Value> = plan.shifts[s]
        .visits
        .iter()
        .zip(&route.elements)
        .map(|(&v, drawn)| {
            let vf = &facts.visits[v];
            let at = drawn.at;
            let mut item = json!({
                "id": vf.id,
                "kind": if vf.is_break() { "BREAK" } else { "VISIT" },
            });
            if !drawn.untimed {
                item["arrivalTime"] = json!(format_date_time(at.arrival));
                item["startServiceTime"] = json!(format_date_time(at.start));
                item["departureTime"] = json!(format_date_time(at.end));
                item["travelSeconds"] = json!(at.travel);
                item["travelMeters"] = json!(at.distance);
                if !vf.is_break() {
                    item["effectiveServiceSeconds"] = json!(at.service);
                }
            }
            if let Some((_, start)) = vf.pin {
                item["pin"] = json!(true);
                item["startServiceTime"] = json!(format_date_time(start));
            }
            if at.unreachable {
                item["unreachable"] = json!(true);
            }
            item
        })
        .collect();
    json["itinerary"] = Value::Array(itinerary);
    if route.active {
        json["metrics"] = json!({
            "startTime": format_date_time(route.start),
            "endLocationArrivalTime": format_date_time(route.end_arrival),
            "lastVisitDepartureTime": format_date_time(route.last_departure),
            "travelSeconds": route.travel,
            "travelMeters": route.distance,
            "serviceSeconds": route.service,
            "waitingSeconds": route.waiting,
            "breakSeconds": route.breaks,
            "overtimeSeconds": route.overtime,
            "cost": route.cost as f64 / 100.0,
        });
        if route.unreachable_legs > 0 {
            json["unreachable"] = json!(true);
        }
    }
}

pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    if plan.facts.shifts.is_empty() {
        return value;
    }
    let plan = timed(plan);
    let routes = crate::route::routes(&plan);
    let mut s = 0;
    if let Some(vehicles) = value.get_mut("vehicles").and_then(Value::as_array_mut) {
        for vehicle in vehicles.iter_mut() {
            if let Some(shifts) = vehicle.get_mut("shifts").and_then(Value::as_array_mut) {
                for json in shifts {
                    write(&plan, s, &routes[s], json);
                    s += 1;
                }
            }
        }
        for vehicle in vehicles.iter_mut() {
            if let Some(shifts) = vehicle
                .get_mut("linkedShifts")
                .and_then(Value::as_array_mut)
            {
                for json in shifts {
                    write(&plan, s, &routes[s], json);
                    s += 1;
                }
            }
        }
    }
    value["unassignedVisits"] = json!(plan
        .facts
        .visits
        .iter()
        .zip(&plan.elements)
        .filter(|(f, e)| !f.is_break() && !e.planned())
        .map(|(f, _)| f.id.clone())
        .collect::<Vec<_>>());
    value
}
