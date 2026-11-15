//! The plan written back in the dataset's own format.

use serde_json::Value;

use crate::domain::Plan;
use crate::time::format_date;

/// The plan as JSON, with every task's `bay` and `start` set to where and
/// when it runs now (or `null` when it is not planned).
pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    if let Some(tasks) = value.get_mut("tasks").and_then(Value::as_array_mut) {
        for (json, task) in tasks.iter_mut().zip(&plan.tasks) {
            let placed = task.slot_idx.map(|slot| {
                let ctx = &task.ctx;
                (
                    plan.bays[ctx.slot_bay(slot)].id.clone(),
                    format_date(ctx.origin + ctx.slot_day(slot)),
                )
            });
            match placed {
                Some((bay, start)) => {
                    json["bay"] = Value::String(bay);
                    json["start"] = Value::String(start);
                }
                None => {
                    json["bay"] = Value::Null;
                    json["start"] = Value::Null;
                }
            }
        }
    }
    value
}
