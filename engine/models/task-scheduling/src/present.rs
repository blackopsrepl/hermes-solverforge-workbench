//! The plan written back in the dataset's own format, with its schedule.

use serde_json::{json, Value};

use crate::domain::Plan;
use crate::schedule::Schedule;
use crate::time::format_date_time;

/// The plan as JSON: each resource lists its tasks in processing order with
/// their start, end and changeover, plus its idle and changeover totals;
/// `unassigned` lists the tasks left open.
pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    if plan.facts.resources.is_empty() && plan.facts.tasks.is_empty() {
        return value;
    }
    let timing = Schedule::of(plan);
    let mut resources = Vec::new();
    let json_resources = value
        .get_mut("resources")
        .and_then(Value::as_array_mut)
        .map(std::mem::take)
        .unwrap_or_default();
    for (r, (mut json, resource)) in json_resources.into_iter().zip(&plan.resources).enumerate() {
        if !resource.is_resource() {
            continue;
        }
        let Some(order) = timing.order.get(r) else {
            resources.push(json);
            continue;
        };
        let mut changeovers = 0;
        let mut changeover_minutes = 0;
        let mut idle = 0;
        let tasks: Vec<Value> = order
            .iter()
            .map(|&t| {
                let mut entry = json!({
                    "task": plan.tasks[t].id,
                    "start": format_date_time(timing.start[t]),
                    "end": format_date_time(timing.end[t]),
                });
                if let Some(changeover) = timing.changeover[t] {
                    changeovers += 1;
                    changeover_minutes += changeover.minutes;
                    entry["changeoverStart"] = json!(format_date_time(changeover.start));
                    entry["changeoverDuration"] = json!(changeover.minutes);
                }
                idle += timing.idle[t];
                entry
            })
            .collect();
        json["tasks"] = Value::Array(tasks);
        if plan.facts.resources[r].machine {
            json["idleTime"] = json!(idle);
            json["changeovers"] = json!(changeovers);
            json["changeoverTime"] = json!(changeover_minutes);
        }
        resources.push(json);
    }
    value["resources"] = Value::Array(resources);
    if let Some(tasks) = value.get_mut("tasks").and_then(Value::as_array_mut) {
        for (json, task) in tasks.iter_mut().zip(&plan.tasks) {
            if task.pinned {
                json["pinned"] = json!(true);
            }
        }
    }
    value["unassigned"] = json!((0..plan.tasks.len())
        .filter(|&t| !timing.assigned(t))
        .map(|t| plan.tasks[t].id.clone())
        .collect::<Vec<_>>());
    value
}
