//! Technicians a crew needs on one day.

use solverforge::stream::collector::{Accumulator, Collector};

use super::Row;
use crate::derived::Ctx;
use crate::wire::Parameters;

/// The summed technicians of a crew-day bucket.
pub struct Load {
    ctx: Option<Ctx>,
    total: i64,
}

impl Load {
    pub fn params(&self) -> Option<&Parameters> {
        self.ctx.as_deref().map(|d| &d.params)
    }
}

/// Collects crew-day rows into a [`Load`].
pub struct LoadCollector;

impl<'i> Collector<&'i Row> for LoadCollector {
    type Value = (Ctx, i64);
    type Result = Load;
    type Accumulator = LoadAccumulator;

    fn extract(&self, row: &'i Row) -> (Ctx, i64) {
        (row.ctx.clone(), row.value)
    }

    fn create_accumulator(&self) -> LoadAccumulator {
        LoadAccumulator(Load {
            ctx: None,
            total: 0,
        })
    }
}

pub struct LoadAccumulator(Load);

impl Accumulator<(Ctx, i64), Load> for LoadAccumulator {
    type Retraction = i64;

    fn accumulate(&mut self, (ctx, technicians): (Ctx, i64)) -> i64 {
        self.0.ctx.get_or_insert(ctx);
        self.0.total += technicians;
        technicians
    }

    fn retract(&mut self, technicians: i64) {
        self.0.total -= technicians;
    }

    fn with_result<T>(&self, f: impl FnOnce(&Load) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.total = 0;
    }
}

/// A crew-day's demand against its strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CrewLoad {
    /// Technicians needed beyond regular strength and overtime.
    pub beyond: i64,
    /// Technician-days of overtime used.
    pub overtime: i64,
    /// Technicians needed above the crew's even share of the work.
    pub above_level: i64,
}

/// The load of crew-day `bucket`.
pub fn crew_load(bucket: usize, load: &Load) -> CrewLoad {
    let Some(d) = load.ctx.as_deref() else {
        return CrewLoad::default();
    };
    let days = d.days as usize;
    let crew = &d.crews[bucket / days];
    let regular = crew.regular[bucket % days];
    let total = load.total;
    CrewLoad {
        beyond: (total - regular - crew.overtime).max(0),
        overtime: (total.min(regular + crew.overtime) - regular).max(0),
        above_level: (total - crew.level).max(0),
    }
}
