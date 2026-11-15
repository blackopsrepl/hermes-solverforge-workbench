//! What one task placed in one slot means: its days, and the per-task rules
//! it breaks there. Every function takes the task index and its slot.

use super::Derived;
use crate::time::overlap;

impl Derived {
    /// Days the task occupies its bay: `(first, end)`.
    pub fn occupied(&self, task: usize, slot: usize) -> (i64, i64) {
        let start = self.slot_day(slot);
        (start, start + self.tasks[task].days)
    }

    /// Days the unit is out of service for the task, return to service included.
    pub fn out_of_service(&self, task: usize, slot: usize) -> (i64, i64) {
        let (start, end) = self.occupied(task, slot);
        (start, end + self.tasks[task].buffer)
    }

    /// Days inside the horizon the task keeps its unit from operating: out
    /// of service for the work, and grounded from its due limit until the
    /// work starts (to the horizon end when it is not planned).
    pub fn down(&self, task: usize, slot: Option<usize>) -> (i64, i64) {
        let info = &self.tasks[task];
        let (first, end) = match slot {
            Some(slot) => {
                let (start, end) = self.out_of_service(task, slot);
                (info.grounded_from.map_or(start, |g| g.min(start)), end)
            }
            None => match info.grounded_from {
                Some(grounded) => (grounded, self.days),
                None => (0, 0),
            },
        };
        (first.max(0), end.min(self.days))
    }

    /// Days the task starts before its earliest start.
    pub fn early_days(&self, task: usize, slot: usize) -> i64 {
        (self.tasks[task].earliest - self.slot_day(slot)).max(0)
    }

    /// Days the task finishes after its due date.
    pub fn late_days(&self, task: usize, slot: usize) -> i64 {
        let (_, end) = self.occupied(task, slot);
        self.tasks[task]
            .finish_by
            .map_or(0, |finish_by| (end - finish_by).max(0))
    }

    /// Days the task starts after its unit reaches the usage limit.
    pub fn usage_days(&self, task: usize, slot: usize) -> i64 {
        let start = self.slot_day(slot);
        self.tasks[task]
            .start_by
            .map_or(0, |start_by| (start - start_by).max(0))
    }

    /// The task's days in a bay it cannot be done in.
    pub fn unsuitable_days(&self, task: usize, slot: usize) -> i64 {
        let info = &self.tasks[task];
        if info.suitable[self.slot_bay(slot)] {
            0
        } else {
            info.days
        }
    }

    /// The task's days in its bay while the bay is closed.
    pub fn closed_days(&self, task: usize, slot: usize) -> i64 {
        let (start, end) = self.occupied(task, slot);
        self.bays[self.slot_bay(slot)]
            .closures
            .iter()
            .map(|&(from, to)| overlap(start, end, from, to))
            .sum()
    }

    /// Days the unit is out of service for the task while it is away or
    /// committed to operations.
    pub fn away_days(&self, task: usize, slot: usize) -> i64 {
        let (start, end) = self.out_of_service(task, slot);
        let unit = &self.units[self.tasks[task].unit];
        let before = overlap(start, end, i64::MIN / 2, unit.available_from);
        let committed: i64 = unit
            .commitments
            .iter()
            .map(|&(from, to)| overlap(start, end, from, to))
            .sum();
        before + committed
    }

    /// What the task's bay costs for its days.
    pub fn bay_cost(&self, task: usize, slot: usize) -> i64 {
        self.bays[self.slot_bay(slot)].cost_per_day * self.tasks[task].days
    }

    /// Whether the task left the bay and day it was published with.
    pub fn moved(&self, task: usize, slot: Option<usize>) -> bool {
        self.tasks[task]
            .published
            .is_some_and(|published| slot != Some(published))
    }

    /// The task's unit's priority.
    pub fn priority(&self, task: usize) -> i64 {
        self.units[self.tasks[task].unit].priority
    }
}
