//! Freezing: the part of the schedule that has started stays as it is.

use solverforge::PlanningSolution;
use solverforge_workbench_host::EngineError;

use crate::domain::Plan;
use crate::time::parse_date_time;

/// Pins every planned task that starts before `until` to its resource, its
/// position and its start; returns how many tasks it pinned. A resource's
/// tasks run in order, so the pinned ones form a prefix of its sequence and
/// join its frozen tasks, and the frozen lane.
pub fn freeze(plan: &mut Plan, until: &str) -> Result<usize, EngineError> {
    let until = parse_date_time(until).ok_or_else(|| {
        EngineError::new(
            "invalid_freeze",
            format!("freeze_before {until:?} is not a local date-time YYYY-MM-DDTHH:MM"),
        )
    })?;
    let frozen_lane = plan.facts.frozen;
    let mut pinned = 0;
    for r in 0..plan.facts.resources.len() {
        let count = plan.resources[r]
            .sequence
            .iter()
            .take_while(|&&t| plan.tasks[t].slot.is_some_and(|slot| slot.start < until))
            .count();
        let moved: Vec<usize> = plan.resources[r].sequence.drain(..count).collect();
        for t in moved {
            let start = plan.tasks[t].slot.map(|slot| slot.start);
            if let Some(&last) = plan.resources[r].frozen.last() {
                plan.tasks[last].frozen_last = false;
            }
            let task = &mut plan.tasks[t];
            task.pinned = true;
            task.frozen_last = true;
            task.pinned_start = start;
            task.frozen_on = Some(r);
            task.frozen_previous = plan.resources[r].frozen.last().copied();
            task.frozen_rank = plan.resources[r].frozen.len();
            plan.resources[r].frozen.push(t);
            plan.resources[frozen_lane].sequence.push(t);
            pinned += 1;
        }
    }
    plan.update_all_shadows();
    Ok(pinned)
}
