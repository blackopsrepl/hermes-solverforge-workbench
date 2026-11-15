//! All tasks of one unit, for rules over a unit's whole workshop programme.

use solverforge::stream::collector::{Accumulator, Collector};

use crate::derived::{Ctx, Derived};
use crate::domain::Task;
use crate::time::overlap;
use crate::wire::Parameters;

/// A unit's tasks as `(task, slot)`, assigned ones by start, open ones last.
pub struct Roster {
    ctx: Option<Ctx>,
    tasks: Vec<(u32, Option<u32>)>,
}

impl Roster {
    pub fn params(&self) -> Option<&Parameters> {
        self.ctx.as_deref().map(|d| &d.params)
    }

    fn key(d: &Derived, (task, slot): (u32, Option<u32>)) -> (i64, u32) {
        (
            slot.map_or(i64::MAX, |slot| d.slot_day(slot as usize)),
            task,
        )
    }

    /// Assigned tasks as `(task, slot)`, by start.
    fn assigned(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.tasks
            .iter()
            .filter_map(|&(task, slot)| slot.map(|slot| (task as usize, slot as usize)))
    }

    fn slot_of(&self, task: usize) -> Option<Option<usize>> {
        self.tasks
            .iter()
            .find(|&&(t, _)| t as usize == task)
            .map(|&(_, slot)| slot.map(|s| s as usize))
    }
}

/// Collects a unit's tasks into a [`Roster`].
pub struct RosterCollector;

impl<'i> Collector<&'i Task> for RosterCollector {
    type Value = (Ctx, (u32, Option<u32>));
    type Result = Roster;
    type Accumulator = RosterAccumulator;

    fn extract(&self, task: &'i Task) -> (Ctx, (u32, Option<u32>)) {
        (
            task.ctx.clone(),
            (task.index as u32, task.slot_idx.map(|s| s as u32)),
        )
    }

    fn create_accumulator(&self) -> RosterAccumulator {
        RosterAccumulator(Roster {
            ctx: None,
            tasks: Vec::new(),
        })
    }
}

pub struct RosterAccumulator(Roster);

impl Accumulator<(Ctx, (u32, Option<u32>)), Roster> for RosterAccumulator {
    type Retraction = u32;

    fn accumulate(&mut self, (ctx, entry): (Ctx, (u32, Option<u32>))) -> u32 {
        let ctx = self.0.ctx.get_or_insert(ctx);
        let d: &Derived = ctx;
        let key = Roster::key(d, entry);
        let at = self
            .0
            .tasks
            .partition_point(|&other| Roster::key(d, other) < key);
        self.0.tasks.insert(at, entry);
        entry.0
    }

    fn retract(&mut self, task: u32) {
        if let Some(at) = self.0.tasks.iter().position(|&(t, _)| t == task) {
            self.0.tasks.remove(at);
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&Roster) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.tasks.clear();
    }
}

/// Days the unit spends in two different bays at once.
pub fn double_booked_days(roster: &Roster) -> i64 {
    let Some(d) = roster.ctx.as_deref() else {
        return 0;
    };
    let assigned: Vec<(usize, usize)> = roster.assigned().collect();
    let mut days = 0;
    for (i, &(a, slot_a)) in assigned.iter().enumerate() {
        let (start_a, end_a) = d.occupied(a, slot_a);
        for &(b, slot_b) in &assigned[i + 1..] {
            let (start_b, end_b) = d.occupied(b, slot_b);
            if start_b >= end_a {
                break;
            }
            if d.slot_bay(slot_a) != d.slot_bay(slot_b) {
                days += overlap(start_a, end_a, start_b, end_b);
            }
        }
    }
    days
}

/// Days each planned task starts before a task it follows has finished; a
/// planned task whose predecessor is not planned costs all its days.
pub fn precedence_days(roster: &Roster) -> i64 {
    let Some(d) = roster.ctx.as_deref() else {
        return 0;
    };
    let mut days = 0;
    for (task, slot) in roster.assigned() {
        let start = d.slot_day(slot);
        for &pred in &d.tasks[task].preds {
            days += match roster.slot_of(pred) {
                Some(Some(pred_slot)) => (d.occupied(pred, pred_slot).1 - start).max(0),
                _ => d.tasks[task].days,
            };
        }
    }
    days
}

/// Separate workshop visits of the unit: runs of back-to-back or
/// overlapping tasks in one bay.
pub fn visits(roster: &Roster) -> i64 {
    let Some(d) = roster.ctx.as_deref() else {
        return 0;
    };
    let mut count = 0;
    let mut current: Option<(usize, i64)> = None;
    for (task, slot) in roster.assigned() {
        let (start, end) = d.occupied(task, slot);
        let bay = d.slot_bay(slot);
        match current {
            Some((at, until)) if at == bay && start <= until => {
                current = Some((bay, until.max(end)));
            }
            _ => {
                count += 1;
                current = Some((bay, end));
            }
        }
    }
    count
}
