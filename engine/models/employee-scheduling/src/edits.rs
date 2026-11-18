//! Employee scheduling replanning changes: an employee off on a day, a shift
//! cancelled.

use std::collections::BTreeSet;

use serde_json::{json, Value};
use solverforge_workbench_host::edit::{
    append, by_id, day_label, next_day, unknown, Args, Choice, Compiled, Edit, Field,
};
use solverforge_workbench_host::EngineError;

use crate::domain::Plan;

const MODEL: &str = "employee-scheduling";

fn employees(plan: &Plan) -> Vec<Choice> {
    plan.employees
        .iter()
        .map(|e| Choice::new(&e.id, e.name.clone().unwrap_or_else(|| e.id.clone())))
        .collect()
}

fn days(plan: &Plan) -> Vec<Choice> {
    let dates: BTreeSet<&str> = plan
        .shifts
        .iter()
        .map(|s| s.start.as_str())
        .filter(|t| t.len() >= 10)
        .map(|t| &t[..10])
        .collect();
    dates
        .into_iter()
        .map(|d| Choice::new(d, day_label(d)))
        .collect()
}

fn shift_label(start: &str, end: &str, tags: &[String]) -> String {
    let what = if tags.is_empty() {
        String::new()
    } else {
        format!(" {}", tags.join(", "))
    };
    format!(
        "{} {}–{}{what}",
        day_label(start.get(..10).unwrap_or(start)),
        start.get(11..16).unwrap_or(""),
        end.get(11..16).unwrap_or(""),
    )
}

fn shifts(plan: &Plan) -> Vec<Choice> {
    plan.shifts
        .iter()
        .map(|s| Choice::new(&s.id, shift_label(&s.start, &s.end, &s.tags)))
        .collect()
}

pub fn edits(plan: &Plan) -> Vec<Edit> {
    vec![
        Edit {
            id: "employee_off",
            label: "An employee is off for a day".into(),
            description: "Marks the employee unavailable all day and frees their shifts that day, \
                          even published ones, to be covered again."
                .into(),
            fields: vec![
                Field::choice("employee", "Employee", employees(plan)),
                Field::choice("day", "Day", days(plan)),
            ],
        },
        Edit {
            id: "shift_cancelled",
            label: "A shift is cancelled".into(),
            description: "Removes the shift; whoever held it is free for others.".into(),
            fields: vec![Field::choice("shift", "Shift", shifts(plan))],
        },
    ]
}

pub fn edit(plan: &Plan, id: &str, raw: &Value) -> Result<Compiled, EngineError> {
    let args = Args::new(id, raw);
    match id {
        "employee_off" => {
            let who = args.choice("employee", &employees(plan))?;
            let day = args.choice("day", &days(plan))?;
            let employee = plan
                .employees
                .iter()
                .find(|e| e.id == who)
                .expect("offered");
            let name = employee.name.clone().unwrap_or_else(|| employee.id.clone());
            let next = next_day(day).expect("offered days are dates");
            let mut operations = vec![append(
                &format!("/employees/{}/unavailable", by_id(who)),
                employee.unavailable.len(),
                json!({"from": format!("{day}T00:00"), "to": format!("{next}T00:00")}),
            )];
            let held: Vec<&str> = plan
                .shifts
                .iter()
                .filter(|s| s.employee.as_deref() == Some(who) && s.start.starts_with(day))
                .map(|s| s.id.as_str())
                .collect();
            for shift in plan.shifts.iter().filter(|s| held.contains(&s.id.as_str())) {
                let path = format!("/shifts/{}", by_id(&shift.id));
                if shift.pinned {
                    operations.push(
                        json!({"op": "replace", "path": format!("{path}/pinned"), "value": false}),
                    );
                }
                operations.push(json!({"op": "remove", "path": format!("{path}/employee")}));
            }
            let freed = match held.len() {
                0 => "they held no shift that day".to_string(),
                1 => "1 shift is free to cover".to_string(),
                n => format!("{n} shifts are free to cover"),
            };
            Ok(Compiled {
                operations,
                summary: format!("{name} is off on {}: {freed}.", day_label(day)),
            })
        }
        "shift_cancelled" => {
            let options = shifts(plan);
            let shift = args.choice("shift", &options)?;
            let label = options
                .into_iter()
                .find(|c| c.value == shift)
                .expect("offered")
                .label;
            Ok(Compiled {
                operations: vec![
                    json!({"op": "remove", "path": format!("/shifts/{}", by_id(shift))}),
                ],
                summary: format!("The shift {label} is cancelled."),
            })
        }
        _ => Err(unknown(MODEL, id, &edits(plan))),
    }
}
