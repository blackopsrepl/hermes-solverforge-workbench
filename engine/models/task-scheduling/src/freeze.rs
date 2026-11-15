//! Freezing: the part of the schedule that has started stays as it is.

use solverforge_workbench_host::EngineError;

use crate::domain::Plan;
use crate::schedule::Timing;
use crate::time::parse_date_time;

/// Pins every planned task that starts before `until` to its resource, its
/// position and its start; returns how many tasks it pinned. A resource's
/// tasks run in order, so the pinned ones form a prefix of its sequence,
/// after any task pinned already.
pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let until = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "invalid_freeze",
            format!("freeze_before {until:?} is not a local date-time YYYY-MM-DDTHH:MM"),
        )
    })?;
    let timing = Timing::of(plan);
    let mut pinned = 0;
    for resource in plan.resources.iter_mut().filter(|r| !r.unscheduled) {
        let count = resource
            .sequence
            .iter()
            .take_while(|&&t| timing.start[t] < until)
            .count();
        for t in resource.sequence.drain(..count) {
            resource.frozen.push(t);
            plan.tasks[t].pinned = true;
            plan.tasks[t].pinned_start = Some(timing.start[t]);
            pinned += 1;
        }
    }
    let mut frozen = vec![false; plan.tasks.len()];
    for resource in &plan.resources {
        for &t in &resource.frozen {
            frozen[t] = true;
        }
    }
    plan.task_values.retain(|&t| !frozen[t]);
    Ok(pinned)
}
