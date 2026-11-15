//! The plan written back in the dataset's own format.

use serde_json::Value;

use crate::derived::Task;
use crate::domain::Plan;
use crate::time::format_date_time;

fn id_or_null(id: Option<&String>) -> Value {
    id.map_or(Value::Null, |id| Value::String(id.clone()))
}

/// The plan as JSON with every order's furnace, start and crew, and every
/// roster row's shift, taken from the planning variables.
pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    if let Some(orders) = value.get_mut("workOrders").and_then(Value::as_array_mut) {
        for (json, order) in orders.iter_mut().zip(&plan.work_orders) {
            match order.placed() {
                Some(p) => {
                    json["furnace"] = id_or_null(plan.furnaces.get(p.furnace).map(|f| &f.id));
                    json["start"] = Value::String(format_date_time(order.ctx.origin + p.start));
                }
                None => {
                    json["furnace"] = Value::Null;
                    json["start"] = Value::Null;
                }
            }
            for task in Task::ALL {
                let id = order
                    .op(task)
                    .and_then(|op| plan.operators.get(op))
                    .map(|o| &o.id);
                json[format!("{}Operator", task.key())] = id_or_null(id);
            }
        }
    }
    if let Some(rows) = value.get_mut("roster").and_then(Value::as_array_mut) {
        for (json, row) in rows.iter_mut().zip(&plan.roster) {
            json["shift"] = row
                .kind()
                .map_or(Value::Null, |kind| Value::String(kind.label().to_string()));
        }
    }
    value
}
