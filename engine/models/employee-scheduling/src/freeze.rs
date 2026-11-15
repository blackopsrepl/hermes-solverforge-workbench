//! Freezing: the part of the roster that has already started stays as it is.

use solverforge_workbench_host::EngineError;

use crate::domain::Plan;
use crate::time::parse_date_time;

/// Pins every shift that starts before `until` to its current employee, or
/// keeps it open if it has none; returns how many shifts it pinned.
pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let until = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "invalid_freeze",
            format!("freeze_before {until:?} is not a local date-time YYYY-MM-DDTHH:MM"),
        )
    })?;
    let mut pinned = 0;
    for shift in &mut plan.shifts {
        if shift.pinned || parse_date_time(&shift.start).is_none_or(|start| start >= until) {
            continue;
        }
        shift.pinned = true;
        pinned += 1;
    }
    Ok(pinned)
}
