//! The values each planning variable may take.

use crate::derived::{Derived, Task, Values};
use crate::time::overlap;

/// Whether a furnace can run an order at all: process, setpoint and weight.
pub fn furnace_fits(d: &Derived, furnace: usize, order: usize) -> bool {
    let f = &d.furnaces[furnace];
    let recipe = d.recipe_of(order);
    f.processes & recipe.process.bit() != 0
        && recipe.temperature <= f.max_temperature
        && d.orders[order].load_kg <= f.max_load_kg
}

/// Whether an order may start at `start` on `furnace` without breaking a rule
/// that the order alone decides: release, horizon, downtime and shift changes.
pub fn start_fits(d: &Derived, furnace: usize, order: usize, start: i64) -> bool {
    let end = start + d.recipe_of(order).cycle;
    end <= d.horizon
        && d.orders[order]
            .release
            .is_none_or(|release| start >= release)
        && d.furnaces[furnace]
            .downtime
            .iter()
            .all(|&(from, to)| overlap(start, end, from, to) == 0)
        && Task::ALL.into_iter().all(|task| {
            d.task_window(order, task, start, end)
                .is_none_or(|window| d.owner_of(window).is_some())
        })
}

/// Slot values an order may take, earliest start first.
pub fn order_candidates(d: &Derived, order: usize) -> Vec<usize> {
    let mut values = Vec::new();
    for slot in 0..d.slots {
        let start = slot as i64 * d.step;
        for furnace in 0..d.furnaces.len() {
            if furnace_fits(d, furnace, order) && start_fits(d, furnace, order, start) {
                values.push(d.value(furnace, slot));
            }
        }
    }
    values
}

/// Operators skilled for one task of an order; none when the order has no such task.
pub fn task_candidates(d: &Derived, order: usize, task: Task) -> Values {
    if task == Task::Quench && !d.recipe_of(order).quench {
        return Values::default();
    }
    let needed = d.task_skills(order, task);
    Values::new(
        d.operators
            .iter()
            .enumerate()
            .filter(|(_, op)| op.skills & needed == needed)
            .map(|(index, _)| index)
            .collect(),
    )
}
