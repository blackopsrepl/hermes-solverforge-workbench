//! Names for the records constraint analysis blames, read from the plan.
//!
//! Analysis reports a record's type and its planning id, the `index` field
//! of tasks and resources: its position in the plan. The index finds the
//! record; the label is what a person reads.

use solverforge_workbench_host::EntityName;

use crate::domain::Plan;

/// A task's or a resource's label: its name and its id.
pub fn label(plan: &Plan, type_name: &str, index: usize) -> String {
    let (id, name, extra) = match type_name {
        "Task" => match plan.tasks.get(index) {
            Some(task) => (&task.id, &task.name, task.task_type.as_deref()),
            None => return format!("Task {index}"),
        },
        "Resource" => match plan.resources.get(index) {
            Some(resource) if !resource.is_resource() => {
                return resource.name.clone().unwrap_or_default()
            }
            Some(resource) => (
                &resource.id,
                &resource.name,
                resource.machine_type.as_deref(),
            ),
            None => return format!("Resource {index}"),
        },
        _ => return format!("{type_name} {index}"),
    };
    let mut label = match name {
        Some(name) if name != id => format!("{name} · {id}"),
        _ => id.clone(),
    };
    if let Some(extra) = extra {
        label.push_str(&format!(" · {extra}"));
    }
    label
}

pub fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {
    let index: usize = planning_id.parse().ok()?;
    let id = match type_name {
        "Task" => plan.tasks.get(index)?.id.clone(),
        "Resource" => {
            let resource = plan.resources.get(index)?;
            if !resource.is_resource() {
                return Some(EntityName {
                    id: None,
                    label: resource.name.clone().unwrap_or_default(),
                });
            }
            resource.id.clone()
        }
        _ => return None,
    };
    Some(EntityName {
        id: Some(id),
        label: label(plan, type_name, index),
    })
}
