//! One employee's shifts, kept sorted by start as the solver moves them.

use solverforge::stream::collector::{Accumulator, Collector};

use crate::derived::{Ctx, Derived, ShiftInfo};
use crate::domain::Shift;

/// The shifts assigned to one employee, sorted by start.
pub struct Roster {
    ctx: Option<Ctx>,
    shifts: Vec<u32>,
}

impl Roster {
    /// An employee with no shifts.
    pub fn empty(ctx: Ctx) -> Self {
        Self {
            ctx: Some(ctx),
            shifts: Vec::new(),
        }
    }

    /// The derived facts, unless the roster has never seen a shift or an employee.
    pub fn derived(&self) -> Option<&Derived> {
        self.ctx.as_deref()
    }

    /// Shift indexes, sorted by start.
    pub fn indexes(&self) -> &[u32] {
        &self.shifts
    }

    pub fn len(&self) -> usize {
        self.shifts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.shifts.is_empty()
    }

    /// `(index, info)` of every shift, sorted by start.
    pub fn iter<'a>(&'a self, d: &'a Derived) -> impl Iterator<Item = (usize, &'a ShiftInfo)> + 'a {
        self.shifts
            .iter()
            .map(move |&i| (i as usize, &d.shifts[i as usize]))
    }

    fn position(&self, d: &Derived, index: u32) -> Result<usize, usize> {
        let key = (d.shifts[index as usize].start, index);
        self.shifts
            .binary_search_by_key(&key, |&i| (d.shifts[i as usize].start, i))
    }

    fn insert(&mut self, ctx: Ctx, index: u32) {
        let ctx = self.ctx.get_or_insert(ctx);
        let d: &Derived = ctx;
        let key = (d.shifts[index as usize].start, index);
        let at = self
            .shifts
            .partition_point(|&i| (d.shifts[i as usize].start, i) < key);
        self.shifts.insert(at, index);
    }

    fn remove(&mut self, index: u32) {
        let Some(ctx) = self.ctx.clone() else {
            return;
        };
        if let Ok(at) = self.position(&ctx, index) {
            self.shifts.remove(at);
        }
    }
}

/// Collects a group of shifts into a [`Roster`].
pub struct RosterCollector;

impl<'i> Collector<&'i Shift> for RosterCollector {
    type Value = (Ctx, u32);
    type Result = Roster;
    type Accumulator = RosterAccumulator;

    fn extract(&self, shift: &'i Shift) -> (Ctx, u32) {
        (shift.ctx.clone(), shift.index as u32)
    }

    fn create_accumulator(&self) -> RosterAccumulator {
        RosterAccumulator(Roster {
            ctx: None,
            shifts: Vec::new(),
        })
    }
}

pub struct RosterAccumulator(Roster);

impl Accumulator<(Ctx, u32), Roster> for RosterAccumulator {
    type Retraction = u32;

    fn accumulate(&mut self, (ctx, index): (Ctx, u32)) -> u32 {
        self.0.insert(ctx, index);
        index
    }

    fn retract(&mut self, index: u32) {
        self.0.remove(index);
    }

    fn with_result<T>(&self, f: impl FnOnce(&Roster) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.shifts.clear();
    }
}
