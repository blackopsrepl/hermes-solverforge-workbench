//! Rows the task, crew and monitoring rules group: one per bucket, booking
//! or shift a planning entity contributes.

use solverforge::stream::collector::{Accumulator, Collector};
use solverforge::stream::{Projection, ProjectionSink};

use crate::derived::{Task, BUCKET};
use crate::domain::{Coverage, RosterDay, WorkOrder};
use crate::wire::{Role, Skill};

/// What one 15-minute bucket asks of the monitoring crew, or gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    /// A furnace ramping up after charging.
    Ramp(usize),
    /// A furnace soaking at its setpoint.
    Soak(usize),
    /// An operator busy with a manual task.
    Busy(usize),
    /// A rostered operator who can monitor.
    Watcher,
}

#[derive(Debug, Clone, Copy)]
pub struct WatchRow {
    pub bucket: i64,
    pub watch: Watch,
}

fn buckets(start: i64, end: i64) -> std::ops::RangeInclusive<i64> {
    start.div_euclid(BUCKET)..=(end - 1).div_euclid(BUCKET)
}

/// Monitoring demand of a scheduled order: ramp, soak and its crew's tasks.
pub struct OrderWatch;

impl Projection<WorkOrder> for OrderWatch {
    type Out = WatchRow;
    // A seven-day cycle in 15-minute buckets, its ramp boundary and four tasks.
    const MAX_EMITS: usize = 768;

    fn project<Sink: ProjectionSink<WatchRow>>(&self, order: &WorkOrder, out: &mut Sink) {
        let Some(placed) = order.placed() else {
            return;
        };
        let ramp_end = placed.start + order.ctx.recipe_of(order.index).ramp();
        for (from, to, watch) in [
            (placed.start, ramp_end, Watch::Ramp(placed.furnace)),
            (ramp_end, placed.end, Watch::Soak(placed.furnace)),
        ] {
            if to > from {
                for bucket in buckets(from, to) {
                    out.emit(WatchRow { bucket, watch });
                }
            }
        }
        for task in Task::ALL {
            if let (Some(op), Some((from, to))) = (order.op(task), order.task_window(task)) {
                for bucket in buckets(from, to) {
                    out.emit(WatchRow {
                        bucket,
                        watch: Watch::Busy(op),
                    });
                }
            }
        }
    }
}

/// Monitoring capacity of a rostered operator who can monitor.
pub struct RosterWatch;

impl Projection<RosterDay> for RosterWatch {
    type Out = WatchRow;
    const MAX_EMITS: usize = 34;

    fn project<Sink: ProjectionSink<WatchRow>>(&self, row: &RosterDay, out: &mut Sink) {
        let Some(shift) = row.shift_id() else {
            return;
        };
        if row.ctx.operators[row.operator_idx()].skills & Skill::Monitor.bit() == 0 {
            return;
        }
        let (from, to) = row.ctx.shift_bounds(shift);
        for bucket in buckets(from, to) {
            out.emit(WatchRow {
                bucket,
                watch: Watch::Watcher,
            });
        }
    }
}

/// One bucket's monitoring demand and capacity.
#[derive(Default)]
pub struct Monitor {
    ramping: Vec<(usize, u32)>,
    soaking: Vec<(usize, u32)>,
    busy: Vec<(usize, u32)>,
    watchers: i64,
}

impl Monitor {
    /// Operators the bucket needs: one per two ramping furnaces, one per four
    /// soaking ones, plus everyone busy with a manual task.
    pub fn needed(&self) -> i64 {
        let ramp = self.ramping.len() as i64;
        let soak = self.soaking.len() as i64;
        (ramp + 1) / 2 + (soak + 3) / 4 + self.busy.len() as i64
    }

    /// Operators missing in the bucket.
    pub fn shortage(&self) -> i64 {
        if self.ramping.is_empty() && self.soaking.is_empty() && self.busy.is_empty() {
            return 0;
        }
        (self.needed() - self.watchers).max(0)
    }
}

fn add(set: &mut Vec<(usize, u32)>, key: usize) {
    match set.iter_mut().find(|(k, _)| *k == key) {
        Some((_, n)) => *n += 1,
        None => set.push((key, 1)),
    }
}

fn remove(set: &mut Vec<(usize, u32)>, key: usize) {
    if let Some(at) = set.iter().position(|(k, _)| *k == key) {
        set[at].1 -= 1;
        if set[at].1 == 0 {
            set.swap_remove(at);
        }
    }
}

/// Collects one bucket's rows into a [`Monitor`].
pub struct MonitorCollector;

impl<'i> Collector<&'i WatchRow> for MonitorCollector {
    type Value = Watch;
    type Result = Monitor;
    type Accumulator = MonitorAccumulator;

    fn extract(&self, row: &'i WatchRow) -> Watch {
        row.watch
    }

    fn create_accumulator(&self) -> MonitorAccumulator {
        MonitorAccumulator(Monitor::default())
    }
}

pub struct MonitorAccumulator(Monitor);

impl Accumulator<Watch, Monitor> for MonitorAccumulator {
    type Retraction = Watch;

    fn accumulate(&mut self, watch: Watch) -> Watch {
        let m = &mut self.0;
        match watch {
            Watch::Ramp(f) => add(&mut m.ramping, f),
            Watch::Soak(f) => add(&mut m.soaking, f),
            Watch::Busy(op) => add(&mut m.busy, op),
            Watch::Watcher => m.watchers += 1,
        }
        watch
    }

    fn retract(&mut self, watch: Watch) {
        let m = &mut self.0;
        match watch {
            Watch::Ramp(f) => remove(&mut m.ramping, f),
            Watch::Soak(f) => remove(&mut m.soaking, f),
            Watch::Busy(op) => remove(&mut m.busy, op),
            Watch::Watcher => m.watchers -= 1,
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&Monitor) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0 = Monitor::default();
    }
}

/// A task given to an operator, or a shift an operator works:
/// `(operator, shift number, +1 per task or -1000 per shift)`.
#[derive(Debug, Clone, Copy)]
pub struct ShiftWork {
    pub operator: usize,
    pub shift: usize,
    pub delta: i64,
}

/// Every owned task of a scheduled order that has an operator.
pub struct OrderShiftWork;

impl Projection<WorkOrder> for OrderShiftWork {
    type Out = ShiftWork;
    const MAX_EMITS: usize = 4;

    fn project<Sink: ProjectionSink<ShiftWork>>(&self, order: &WorkOrder, out: &mut Sink) {
        for task in Task::ALL {
            let (Some(operator), Some(window)) = (order.op(task), order.task_window(task)) else {
                continue;
            };
            if let Some(shift) = order.ctx.owner_of(window) {
                out.emit(ShiftWork {
                    operator,
                    shift: shift.number(),
                    delta: 1,
                });
            }
        }
    }
}

/// The shift an operator is rostered on.
pub struct RosterShiftWork;

impl Projection<RosterDay> for RosterShiftWork {
    type Out = ShiftWork;
    const MAX_EMITS: usize = 1;

    fn project<Sink: ProjectionSink<ShiftWork>>(&self, row: &RosterDay, out: &mut Sink) {
        if let Some(shift) = row.shift_id() {
            out.emit(ShiftWork {
                operator: row.operator_idx(),
                shift: shift.number(),
                delta: -1000,
            });
        }
    }
}

/// Demand for, or supply of, one role on one shift.
#[derive(Debug, Clone, Copy)]
pub struct Staffing {
    pub shift: usize,
    pub role: Role,
    pub delta: i64,
}

/// A coverage demand: `+count`.
pub struct Demand;

impl Projection<Coverage> for Demand {
    type Out = Staffing;
    const MAX_EMITS: usize = 1;

    fn project<Sink: ProjectionSink<Staffing>>(&self, demand: &Coverage, out: &mut Sink) {
        out.emit(Staffing {
            shift: demand.shift_number,
            role: demand.role,
            delta: demand.count,
        });
    }
}

/// A rostered operator: `-1` for their role on their shift.
pub struct Supply;

impl Projection<RosterDay> for Supply {
    type Out = Staffing;
    const MAX_EMITS: usize = 1;

    fn project<Sink: ProjectionSink<Staffing>>(&self, row: &RosterDay, out: &mut Sink) {
        if let Some(shift) = row.shift_id() {
            out.emit(Staffing {
                shift: shift.number(),
                role: row.ctx.operators[row.operator_idx()].role,
                delta: -1,
            });
        }
    }
}

/// One manual task an operator is booked for.
#[derive(Debug, Clone, Copy)]
pub struct Booking {
    pub order: usize,
    pub task: Task,
    pub operator: usize,
    pub start: i64,
    pub end: i64,
    /// Slot value and recipe of the load, for tasks done once per load.
    pub load: (usize, usize),
}

impl Booking {
    pub fn overlap(&self, other: &Booking) -> i64 {
        crate::time::overlap(self.start, self.end, other.start, other.end)
    }

    /// The same task of one shared load: one job, not two.
    pub fn same_job(&self, other: &Booking) -> bool {
        self.task == other.task && self.task.per_load() && self.load == other.load
    }
}

/// Every task of a scheduled order that has an operator.
pub struct Bookings;

impl Projection<WorkOrder> for Bookings {
    type Out = Booking;
    const MAX_EMITS: usize = 4;

    fn project<Sink: ProjectionSink<Booking>>(&self, order: &WorkOrder, out: &mut Sink) {
        let Some(slot) = order.slot else {
            return;
        };
        for task in Task::ALL {
            if let (Some(operator), Some((start, end))) = (order.op(task), order.task_window(task))
            {
                out.emit(Booking {
                    order: order.index,
                    task,
                    operator,
                    start,
                    end,
                    load: (slot, order.recipe_idx()),
                });
            }
        }
    }
}

/// Night shifts per window of `max + 1` days:
/// `(operator, first day of window, most nights in a row)`.
pub struct NightWindows;

impl Projection<RosterDay> for NightWindows {
    type Out = (usize, i64, i64);
    const MAX_EMITS: usize = 8;

    fn project<Sink: ProjectionSink<(usize, i64, i64)>>(&self, row: &RosterDay, out: &mut Sink) {
        if row.day_index() < 0 || row.kind() != Some(crate::wire::ShiftKind::Night) {
            return;
        }
        let day = row.day_index();
        let span = row.ctx.s.max_consecutive_nights;
        for first in (day - span).max(0)..=day {
            out.emit((row.operator_idx(), first, span));
        }
    }
}
