//! The slots the solver tries for each task.

use crate::derived::Derived;
use crate::domain::Plan;

/// Suitable bays on every day the task can start inside its window without
/// a closed bay or an away unit, ordered by day then bay. When no such slot
/// exists, every suitable bay on every day from the earliest start: the task
/// can then still be planned, late, rather than not at all.
pub fn candidates(d: &Derived, task: usize) -> Vec<usize> {
    let info = &d.tasks[task];
    let last_start = d.days - 1;
    let latest = [
        info.finish_by.map(|f| f - info.days),
        info.start_by,
        Some(last_start),
    ]
    .into_iter()
    .flatten()
    .min()
    .unwrap_or(last_start);
    let slots = |to: i64, clean: bool| -> Vec<usize> {
        let mut out = Vec::new();
        for day in info.earliest..=to {
            for (bay, suitable) in info.suitable.iter().enumerate() {
                if !suitable {
                    continue;
                }
                let slot = d.slot(bay, day);
                if clean && (d.closed_days(task, slot) > 0 || d.away_days(task, slot) > 0) {
                    continue;
                }
                out.push(slot);
            }
        }
        out
    };
    let inside = slots(latest, true);
    if inside.is_empty() {
        slots(last_start, false)
    } else {
        inside
    }
}

/// Drops every candidate slot of an unpinned task that starts before `day`:
/// the frozen past takes no new work.
pub fn restrict_from(plan: &mut Plan, day: i64) {
    for task in plan.tasks.iter_mut().filter(|t| !t.pinned) {
        let ctx = task.ctx.clone();
        task.candidates.retain(|&slot| ctx.slot_day(slot) >= day);
    }
}
