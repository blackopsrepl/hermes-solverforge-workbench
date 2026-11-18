//! Task scheduling replanning changes: a machine or person unavailable for a
//! while, a task due at a new time.

use serde_json::{json, Value};
use solverforge_workbench_host::edit::{
    append, by_id, unknown, Args, Choice, Compiled, Edit, Field, Kind,
};
use solverforge_workbench_host::EngineError;

use crate::domain::Plan;

const MODEL: &str = "task-scheduling";

fn resources(plan: &Plan) -> Vec<Choice> {
    plan.resources
        .iter()
        .map(|r| Choice::new(&r.id, r.name.clone().unwrap_or_else(|| r.id.clone())))
        .collect()
}

fn tasks(plan: &Plan) -> Vec<Choice> {
    plan.tasks
        .iter()
        .map(|t| Choice::new(&t.id, t.name.clone().unwrap_or_else(|| t.id.clone())))
        .collect()
}

pub fn edits(plan: &Plan) -> Vec<Edit> {
    vec![
        Edit {
            id: "resource_down",
            label: "A machine or person is unavailable".into(),
            description: "Nothing runs on it in that time; its tasks then are planned around it."
                .into(),
            fields: vec![
                Field::choice("resource", "Machine or person", resources(plan)),
                Field::new("from", "From", Kind::DateTime),
                Field::new("until", "Until", Kind::DateTime),
                Field::new("reason", "Why", Kind::Text).optional(),
            ],
        },
        Edit {
            id: "task_due",
            label: "A task is due at a new time".into(),
            description: "Finishing after it counts as lateness.".into(),
            fields: vec![
                Field::choice("task", "Task", tasks(plan)),
                Field::new("due", "Due", Kind::DateTime),
            ],
        },
    ]
}

pub fn edit(plan: &Plan, id: &str, raw: &Value) -> Result<Compiled, EngineError> {
    let args = Args::new(id, raw);
    let when = |t: &str| t.replace('T', " ");
    match id {
        "resource_down" => {
            let options = resources(plan);
            let which = args.choice("resource", &options)?;
            let (from, until) = (args.date_time("from")?, args.date_time("until")?);
            if from >= until {
                return Err(EngineError::new(
                    "invalid_edit",
                    "the time must end after it starts",
                ));
            }
            let resource = plan
                .resources
                .iter()
                .find(|r| r.id == which)
                .expect("offered");
            let mut span = json!({"start": from, "end": until});
            if let Some(reason) = args.text_opt("reason") {
                span["label"] = json!(reason);
            }
            let label = options
                .into_iter()
                .find(|c| c.value == which)
                .expect("offered")
                .label;
            Ok(Compiled {
                operations: vec![append(
                    &format!("/resources/{}/unavailable", by_id(which)),
                    resource.unavailable.len(),
                    span,
                )],
                summary: format!(
                    "{label} is unavailable from {} to {}.",
                    when(&from),
                    when(&until)
                ),
            })
        }
        "task_due" => {
            let options = tasks(plan);
            let which = args.choice("task", &options)?;
            let due = args.date_time("due")?;
            let task = plan.tasks.iter().find(|t| t.id == which).expect("offered");
            let label = options
                .into_iter()
                .find(|c| c.value == which)
                .expect("offered")
                .label;
            let op = if task.due_date.is_some() {
                "replace"
            } else {
                "add"
            };
            Ok(Compiled {
                operations: vec![
                    json!({"op": op, "path": format!("/tasks/{}/dueDate", by_id(which)), "value": due}),
                ],
                summary: format!("{label} is due {}.", when(&due)),
            })
        }
        _ => Err(unknown(MODEL, id, &edits(plan))),
    }
}
