//! Projections that spread one task into the buckets it touches.

use solverforge::prelude::{Projection, ProjectionSink};

use crate::derived::Ctx;
use crate::domain::Task;
use crate::prepare::{MAX_HORIZON_DAYS, MAX_LABOUR, MAX_PARTS, MAX_TASK_DAYS};

/// One task in one bucket: `value` is a unit, a technician count or a
/// quantity, depending on the projection; `day` is the task's start.
pub struct Row {
    pub ctx: Ctx,
    pub bucket: u32,
    pub task: u32,
    pub value: i64,
    pub day: i64,
}

fn row(task: &Task, bucket: usize, value: i64, day: i64) -> Row {
    Row {
        ctx: task.ctx.clone(),
        bucket: bucket as u32,
        task: task.index as u32,
        value,
        day,
    }
}

/// Each day the task keeps its unit from operating, with the unit: in the
/// workshop, returning to service, or grounded past a due limit.
pub struct ByDown;

impl Projection<Task> for ByDown {
    type Out = Row;
    const MAX_EMITS: usize = MAX_HORIZON_DAYS as usize;

    fn project<Sink: ProjectionSink<Row>>(&self, task: &Task, sink: &mut Sink) {
        let ctx = &task.ctx;
        let (first, end) = ctx.down(task.index, task.slot_idx);
        let unit = ctx.task(task.index).unit as i64;
        for day in first..end {
            sink.emit(row(task, day as usize, unit, day));
        }
    }
}

/// Each bay-day an assigned task occupies, with its unit.
pub struct ByBayDay;

impl Projection<Task> for ByBayDay {
    type Out = Row;
    const MAX_EMITS: usize = MAX_TASK_DAYS as usize;

    fn project<Sink: ProjectionSink<Row>>(&self, task: &Task, sink: &mut Sink) {
        let Some(slot) = task.slot_idx else {
            return;
        };
        let ctx = &task.ctx;
        let (start, end) = ctx.occupied(task.index, slot);
        let bay = ctx.slot_bay(slot);
        let unit = ctx.task(task.index).unit as i64;
        for day in start.max(0)..end.min(ctx.days) {
            sink.emit(row(task, ctx.slot(bay, day), unit, start));
        }
    }
}

/// Each crew-day an assigned task works, with its technicians.
pub struct ByCrewDay;

impl Projection<Task> for ByCrewDay {
    type Out = Row;
    const MAX_EMITS: usize = MAX_LABOUR * MAX_TASK_DAYS as usize;

    fn project<Sink: ProjectionSink<Row>>(&self, task: &Task, sink: &mut Sink) {
        let Some(slot) = task.slot_idx else {
            return;
        };
        let ctx = &task.ctx;
        let (start, end) = ctx.occupied(task.index, slot);
        for &(crew, technicians) in &ctx.task(task.index).labour {
            for day in start.max(0)..end.min(ctx.days) {
                sink.emit(row(
                    task,
                    crew * ctx.days as usize + day as usize,
                    technicians,
                    start,
                ));
            }
        }
    }
}

/// Each part an assigned task consumes, with the quantity and its start.
pub struct ByPart;

impl Projection<Task> for ByPart {
    type Out = Row;
    const MAX_EMITS: usize = MAX_PARTS;

    fn project<Sink: ProjectionSink<Row>>(&self, task: &Task, sink: &mut Sink) {
        let Some(slot) = task.slot_idx else {
            return;
        };
        let ctx = &task.ctx;
        let start = ctx.slot_day(slot);
        for &(part, quantity) in &ctx.task(task.index).parts {
            sink.emit(row(task, part, quantity, start));
        }
    }
}
