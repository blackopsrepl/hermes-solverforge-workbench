//! Freezing: work that has already started stays where it is, and nothing
//! new is planned into the frozen past.

use solverforge_workbench_host::EngineError;

use crate::domain::Plan;
use crate::prepare::restrict_from;
use crate::time::{parse_moment, DAY};

/// Pins every planned task that starts before `until` (a local date or
/// date-time; a task starts at the beginning of its first day) to its bay
/// and day, and keeps unpinned tasks from starting before it. Returns how
/// many tasks it pinned.
pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let moment = parse_moment(until).ok_or_else(|| {
        EngineError::new(
            "invalid_freeze",
            format!("freeze_before {until:?} is not a local date YYYY-MM-DD or date-time YYYY-MM-DDTHH:MM"),
        )
    })?;
    let Some(origin) = plan.days.first().map(|day| day.ctx.origin) else {
        return Ok(0);
    };
    // The first day that has not started at `until`.
    let open_from = (moment + DAY - 1).div_euclid(DAY) - origin;
    let mut pinned = 0;
    for task in &mut plan.tasks {
        let Some(slot) = task.slot_idx else {
            continue;
        };
        if task.pinned || task.ctx.slot_day(slot) >= open_from {
            continue;
        }
        task.pinned = true;
        pinned += 1;
    }
    restrict_from(plan, open_from);
    Ok(pinned)
}
