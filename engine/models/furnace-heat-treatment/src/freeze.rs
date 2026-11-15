//! Freezing: what has already started stays as it is.

use solverforge_workbench_host::EngineError;

use crate::derived::ShiftId;
use crate::domain::Plan;
use crate::time::parse_date_time;
use crate::wire::ShiftKind;

/// Pins every load charged before `until` to its furnace, start and crew,
/// and every roster day whose first shift starts before `until` to its
/// shift; returns how many orders and roster days it pinned.
pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let at = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "invalid_freeze",
            format!("freeze_before {until:?} is not a local date-time YYYY-MM-DDTHH:MM"),
        )
    })?;
    let mut pinned = 0;
    for order in &mut plan.work_orders {
        let started = order
            .placed()
            .is_some_and(|p| order.ctx.origin + p.start < at);
        if started && !order.pinned {
            order.pinned = true;
            pinned += 1;
        }
    }
    for row in &mut plan.roster {
        let d = &row.ctx;
        let day = row.day_index();
        let first = row.shift_id().unwrap_or(ShiftId {
            day,
            kind: if day < 0 {
                ShiftKind::Night
            } else {
                ShiftKind::Morning
            },
        });
        if !row.pinned && d.origin + d.shift_bounds(first).0 < at {
            row.pinned = true;
            pinned += 1;
        }
    }
    Ok(pinned)
}
