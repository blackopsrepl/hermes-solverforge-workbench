//! Names for the entities constraint analysis blames, read from the plan.
//!
//! Analysis reports an entity's type name and its planning id: the `index`
//! field of every type here, its position in the plan. The index finds the
//! record; the label is what a person reads.

use solverforge_workbench_host::EntityName;

use crate::domain::Plan;
use crate::labels::{date_text, furnace, moment, operator, recipe};

pub fn name_entity(plan: &Plan, type_name: &str, planning_id: &str) -> Option<EntityName> {
    let index: usize = planning_id.parse().ok()?;
    let named = |id: &str, label: String| {
        Some(EntityName {
            id: Some(id.to_string()),
            label,
        })
    };
    match type_name {
        "WorkOrder" => {
            let order = plan.work_orders.get(index)?;
            let when = match order.placed() {
                Some(p) => format!(
                    "{}, {}",
                    moment(&order.ctx, p.start),
                    furnace(plan, p.furnace)
                ),
                None => "not scheduled".to_string(),
            };
            named(
                &order.id,
                format!("{} · {} · {when}", order.id, recipe(order)),
            )
        }
        "RosterDay" => {
            let row = plan.roster.get(index)?;
            let shift = row.kind().map_or("undecided", |kind| kind.label());
            named(
                &row.id,
                format!(
                    "{} · {} {shift}",
                    operator(plan, row.operator_idx()),
                    date_text(&row.day)
                ),
            )
        }
        "Coverage" => {
            let demand = plan.coverage.get(index)?;
            named(
                &demand.id,
                format!(
                    "{} {} · {} × {}",
                    date_text(&demand.day),
                    demand.shift.label(),
                    demand.count,
                    demand.role.label().to_lowercase()
                ),
            )
        }
        "Furnace" => {
            let f = plan.furnaces.get(index)?;
            named(&f.id, f.label().to_string())
        }
        "Operator" => {
            let o = plan.operators.get(index)?;
            named(&o.id, o.label().to_string())
        }
        _ => None,
    }
}
