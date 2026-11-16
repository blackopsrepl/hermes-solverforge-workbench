//! The schedule read off the task slots, by resource, for presenting,
//! viewing and measuring a plan.

use super::{Changeover, Slot};
use crate::domain::Plan;

/// Marks a task no machine or employee runs.
pub const UNASSIGNED: usize = usize::MAX;

/// Every task's resource and timing, and each resource's tasks in order.
#[derive(Debug, Clone, Default)]
pub struct Schedule {
    /// Resource index per task, or [`UNASSIGNED`].
    pub resource: Vec<usize>,
    /// Position on its resource, frozen tasks first.
    pub position: Vec<usize>,
    pub start: Vec<i64>,
    pub end: Vec<i64>,
    pub changeover: Vec<Option<Changeover>>,
    pub idle: Vec<i64>,
    /// Per resource: its tasks in processing order.
    pub order: Vec<Vec<usize>>,
}

impl Schedule {
    /// The schedule of `plan`, from its slots.
    pub fn of(plan: &Plan) -> Self {
        let facts = &*plan.facts;
        let n = plan.tasks.len();
        let resources = facts.resources.len();
        let mut schedule = Self {
            resource: vec![UNASSIGNED; n],
            position: vec![0; n],
            start: vec![0; n],
            end: vec![0; n],
            changeover: vec![None; n],
            idle: vec![0; n],
            order: vec![Vec::new(); resources],
        };
        for (r, resource) in plan.resources.iter().enumerate().take(resources) {
            let start = facts.resources[r].start;
            for &t in resource.frozen.iter().chain(&resource.sequence) {
                let order = &mut schedule.order[r];
                schedule.resource[t] = r;
                schedule.position[t] = order.len();
                order.push(t);
                // A task that cannot be timed (it waits on a task queued
                // after it) shows at the resource's start.
                let slot = plan.tasks[t].slot.filter(Slot::placed);
                schedule.start[t] = slot.map_or(start, |s| s.start);
                schedule.end[t] = slot.map_or(start, |s| s.end);
                schedule.changeover[t] = slot.and_then(|s| s.changeover);
                schedule.idle[t] = slot.map_or(0, |s| s.idle);
            }
        }
        schedule
    }

    /// Whether task `t` runs on a machine or an employee.
    pub fn assigned(&self, t: usize) -> bool {
        self.resource.get(t).is_some_and(|&r| r != UNASSIGNED)
    }
}
