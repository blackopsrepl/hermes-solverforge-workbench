//! The plan written back in the dataset's own format.

use serde_json::Value;

use crate::domain::Plan;

/// The plan as JSON, with every shift's `employee` set to the id of the
/// employee assigned now (or `null`).
pub fn present(plan: &Plan) -> Value {
    let mut value = serde_json::to_value(plan).unwrap_or_default();
    if let Some(shifts) = value.get_mut("shifts").and_then(Value::as_array_mut) {
        for (json, shift) in shifts.iter_mut().zip(&plan.shifts) {
            let employee = shift
                .employee_idx
                .and_then(|index| plan.employees.get(index))
                .map_or(Value::Null, |employee| Value::String(employee.id.clone()));
            json["employee"] = employee;
        }
    }
    value
}
