//! Names for the records constraint analysis blames, read from the plan.
//!
//! Analysis reports a record's type and its planning id, the `index` field
//! of visits and shifts: its position in the plan. The index finds the
//! record; the label is what a person reads.

use solverforge_workbench_host::EntityName;

use crate::derived::Kind;
use crate::domain::Plan;

/// A visit's or a shift's label.
pub fn label(plan: &Plan, type_name: &str, index: usize) -> String {
    let facts = &plan.facts;
    match type_name {
        "Visit" => match facts.visits.get(index) {
            Some(visit) => match &visit.kind {
                Kind::Stop { job, seq } => {
                    let job = &facts.jobs[*job];
                    let role = match (*seq, job.stops.len()) {
                        (0, _) => "pick-up",
                        (s, n) if s + 1 == n => "drop-off",
                        _ => "stop",
                    };
                    format!("{} · {role} at {}", job.label, visit.label)
                }
                Kind::Break(b) => format!(
                    "{} {} · {}",
                    if b.fixed { "Break" } else { "Floating break" },
                    visit.label,
                    facts.shifts[b.shift].label
                ),
            },
            None => format!("Visit {index}"),
        },
        "Shift" => facts
            .shifts
            .get(index)
            .map_or_else(|| format!("Shift {index}"), |s| s.label.clone()),
        _ => format!("{type_name} {index}"),
    }
}

pub fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {
    let index: usize = planning_id.parse().ok()?;
    let id = match type_name {
        "Visit" => plan.facts.visits.get(index)?.id.clone(),
        "Shift" => {
            let shift = plan.facts.shifts.get(index)?;
            if shift.bench {
                return Some(EntityName {
                    id: None,
                    label: shift.label.clone(),
                });
            }
            shift.id.clone()
        }
        _ => return None,
    };
    Some(EntityName {
        id: Some(id),
        label: label(plan, type_name, index),
    })
}
