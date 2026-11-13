//! Assigned shifts grouped by day or by schedule-wide window, with the
//! employee each one is assigned to.

use solverforge::prelude::{Projection, ProjectionSink};
use solverforge::stream::collector::{Accumulator, Collector};

use crate::derived::{Ctx, Derived, ShiftInfo};
use crate::domain::Shift;
use crate::prepare::MAX_SHIFT_DAYS;

/// One assigned shift in one bucket.
pub struct Row {
    pub ctx: Ctx,
    pub bucket: u32,
    pub shift: u32,
    pub employee: u32,
}

/// Emits an assigned shift into every day bucket that can see it: bucket `d`
/// holds the shifts touching days `d ..= d + MAX_SHIFT_DAYS - 1`, so every
/// shift starting on day `d` meets every shift it overlaps there.
pub struct ByDay;

impl Projection<Shift> for ByDay {
    type Out = Row;
    const MAX_EMITS: usize = 2 * MAX_SHIFT_DAYS as usize - 1;

    fn project<Sink>(&self, shift: &Shift, sink: &mut Sink)
    where
        Sink: ProjectionSink<Row>,
    {
        let Some(employee) = shift.employee_idx else {
            return;
        };
        let info = shift.ctx.shift(shift.index);
        let from = (info.day - (MAX_SHIFT_DAYS - 1)).max(0);
        let to = info
            .last_day
            .min(info.day + MAX_SHIFT_DAYS - 1)
            .min(shift.ctx.days - 1);
        for day in from..=to {
            sink.emit(Row {
                ctx: shift.ctx.clone(),
                bucket: day as u32,
                shift: shift.index as u32,
                employee: employee as u32,
            });
        }
    }
}

/// Emits an assigned shift into every schedule-wide window it counts toward.
pub struct ByWindow;

impl Projection<Shift> for ByWindow {
    type Out = Row;
    const MAX_EMITS: usize = crate::prepare::MAX_WINDOWS_PER_SHIFT;

    fn project<Sink>(&self, shift: &Shift, sink: &mut Sink)
    where
        Sink: ProjectionSink<Row>,
    {
        let Some(employee) = shift.employee_idx else {
            return;
        };
        for &window in &shift.ctx.shift_windows[shift.index] {
            sink.emit(Row {
                ctx: shift.ctx.clone(),
                bucket: window,
                shift: shift.index as u32,
                employee: employee as u32,
            });
        }
    }
}

/// The assigned shifts of one bucket as `(shift, employee)`, sorted by start.
pub struct Members {
    ctx: Option<Ctx>,
    rows: Vec<(u32, u32)>,
}

impl Members {
    pub fn empty(ctx: Ctx) -> Self {
        Self {
            ctx: Some(ctx),
            rows: Vec::new(),
        }
    }

    pub fn derived(&self) -> Option<&Derived> {
        self.ctx.as_deref()
    }

    /// `(shift index, employee index, info)`, sorted by start.
    pub fn iter<'a>(
        &'a self,
        d: &'a Derived,
    ) -> impl Iterator<Item = (usize, usize, &'a ShiftInfo)> + 'a {
        self.rows
            .iter()
            .map(move |&(s, e)| (s as usize, e as usize, &d.shifts[s as usize]))
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Collects bucket rows into [`Members`].
pub struct MembersCollector;

impl<'i> Collector<&'i Row> for MembersCollector {
    type Value = (Ctx, u32, u32);
    type Result = Members;
    type Accumulator = MembersAccumulator;

    fn extract(&self, row: &'i Row) -> (Ctx, u32, u32) {
        (row.ctx.clone(), row.shift, row.employee)
    }

    fn create_accumulator(&self) -> MembersAccumulator {
        MembersAccumulator(Members {
            ctx: None,
            rows: Vec::new(),
        })
    }
}

pub struct MembersAccumulator(Members);

impl Accumulator<(Ctx, u32, u32), Members> for MembersAccumulator {
    type Retraction = u32;

    fn accumulate(&mut self, (ctx, shift, employee): (Ctx, u32, u32)) -> u32 {
        let ctx = self.0.ctx.get_or_insert(ctx);
        let d: &Derived = ctx;
        let key = (d.shifts[shift as usize].start, shift);
        let at = self
            .0
            .rows
            .partition_point(|&(s, _)| (d.shifts[s as usize].start, s) < key);
        self.0.rows.insert(at, (shift, employee));
        shift
    }

    fn retract(&mut self, shift: u32) {
        if let Some(at) = self.0.rows.iter().position(|&(s, _)| s == shift) {
            self.0.rows.remove(at);
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&Members) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.rows.clear();
    }
}

/// Emits an assigned alternative shift into its alternatives group.
pub struct ByChoice;

impl Projection<Shift> for ByChoice {
    type Out = Row;
    const MAX_EMITS: usize = 1;

    fn project<Sink>(&self, shift: &Shift, sink: &mut Sink)
    where
        Sink: ProjectionSink<Row>,
    {
        let (Some(employee), Some(group)) =
            (shift.employee_idx, shift.ctx.shift(shift.index).alternative)
        else {
            return;
        };
        sink.emit(Row {
            ctx: shift.ctx.clone(),
            bucket: group,
            shift: shift.index as u32,
            employee: employee as u32,
        });
    }
}

/// Emits every assigned shift of an employee in a cost group with a share
/// into the single whole-schedule bucket.
pub struct ByShare;

impl Projection<Shift> for ByShare {
    type Out = Row;
    const MAX_EMITS: usize = 1;

    fn project<Sink>(&self, shift: &Shift, sink: &mut Sink)
    where
        Sink: ProjectionSink<Row>,
    {
        let Some(employee) = shift.employee_idx else {
            return;
        };
        if shift.ctx.employees[employee].share_group.is_none() {
            return;
        }
        sink.emit(Row {
            ctx: shift.ctx.clone(),
            bucket: 0,
            shift: shift.index as u32,
            employee: employee as u32,
        });
    }
}
