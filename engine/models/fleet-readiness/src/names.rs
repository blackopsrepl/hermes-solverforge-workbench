//! Names for the entities constraint analysis blames, read from the plan.
//!
//! Analysis reports an entity's type name and its planning id: the `index`
//! field of every type here, which is its position in the plan. The index
//! finds the record; the label is what a person reads: the unit and the work
//! with its days, a bay and a day, a date.

use solverforge_workbench_host::EntityName;

use crate::domain::Plan;
use crate::time::{day_label, span_label};

pub fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {
    let index: usize = planning_id.parse().ok()?;
    match type_name {
        "Task" => {
            let task = plan.tasks.get(index)?;
            Some(EntityName {
                id: Some(task.id.clone()),
                label: task_label(plan, index),
            })
        }
        "BaySlot" => {
            let slot = plan.slots.get(index)?;
            let ctx = &slot.ctx;
            let bay = plan.bays.get(ctx.slot_bay(index))?;
            Some(EntityName {
                id: Some(slot_id(plan, index)),
                label: format!(
                    "{} · {}",
                    bay.name.as_deref().unwrap_or(&bay.id),
                    day_label(ctx.origin + ctx.slot_day(index))
                ),
            })
        }
        "Day" => {
            let day = plan.days.get(index)?;
            Some(EntityName {
                id: Some(crate::time::format_date(day.ctx.origin + index as i64)),
                label: day_label(day.ctx.origin + index as i64),
            })
        }
        _ => None,
    }
}

/// The wire id of a slot: `<bay id>@<YYYY-MM-DD>`.
pub fn slot_id(plan: &Plan, slot: usize) -> String {
    let ctx = &plan.slots[slot].ctx;
    format!(
        "{}@{}",
        plan.bays[ctx.slot_bay(slot)].id,
        crate::time::format_date(ctx.origin + ctx.slot_day(slot))
    )
}

/// `Unit name · work · days` (or `· unplanned`).
pub fn task_label(plan: &Plan, index: usize) -> String {
    let task = &plan.tasks[index];
    let unit = plan
        .units
        .iter()
        .find(|unit| unit.id == task.unit)
        .map_or(task.unit.as_str(), |unit| {
            unit.name.as_deref().unwrap_or(&unit.id)
        });
    let when = match task.slot_idx {
        Some(slot) => {
            let ctx = &task.ctx;
            let (start, end) = ctx.occupied(index, slot);
            span_label(ctx.origin + start, ctx.origin + end)
        }
        None => "unplanned".to_string(),
    };
    format!("{unit} · {} · {when}", task.name)
}
