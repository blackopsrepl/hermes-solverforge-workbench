//! The units in one bucket: down on a day, or in a bay on a day.

use solverforge::stream::collector::{Accumulator, Collector};

use super::Row;
use crate::derived::{Ctx, Derived};

/// Units of a bucket with how many rows name each, by unit.
pub struct UnitSet {
    ctx: Option<Ctx>,
    counts: Vec<(u32, u32)>,
}

impl UnitSet {
    pub fn empty(ctx: Ctx) -> Self {
        Self {
            ctx: Some(ctx),
            counts: Vec::new(),
        }
    }

    pub fn derived(&self) -> Option<&Derived> {
        self.ctx.as_deref()
    }

    /// Distinct units, in index order.
    pub fn units(&self) -> impl Iterator<Item = usize> + '_ {
        self.counts.iter().map(|&(unit, _)| unit as usize)
    }

    pub fn len(&self) -> usize {
        self.counts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

/// Collects rows whose `value` is a unit into a [`UnitSet`].
pub struct UnitSetCollector;

impl<'i> Collector<&'i Row> for UnitSetCollector {
    type Value = (Ctx, u32);
    type Result = UnitSet;
    type Accumulator = UnitSetAccumulator;

    fn extract(&self, row: &'i Row) -> (Ctx, u32) {
        (row.ctx.clone(), row.value as u32)
    }

    fn create_accumulator(&self) -> UnitSetAccumulator {
        UnitSetAccumulator(UnitSet {
            ctx: None,
            counts: Vec::new(),
        })
    }
}

pub struct UnitSetAccumulator(UnitSet);

impl Accumulator<(Ctx, u32), UnitSet> for UnitSetAccumulator {
    type Retraction = u32;

    fn accumulate(&mut self, (ctx, unit): (Ctx, u32)) -> u32 {
        self.0.ctx.get_or_insert(ctx);
        let counts = &mut self.0.counts;
        match counts.binary_search_by_key(&unit, |&(u, _)| u) {
            Ok(at) => counts[at].1 += 1,
            Err(at) => counts.insert(at, (unit, 1)),
        }
        unit
    }

    fn retract(&mut self, unit: u32) {
        let counts = &mut self.0.counts;
        if let Ok(at) = counts.binary_search_by_key(&unit, |&(u, _)| u) {
            counts[at].1 -= 1;
            if counts[at].1 == 0 {
                counts.remove(at);
            }
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&UnitSet) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.counts.clear();
    }
}

/// Units in bay-day `bucket` beyond the bay's capacity.
pub fn bay_excess(bucket: usize, set: &UnitSet) -> i64 {
    let Some(d) = set.derived() else {
        return 0;
    };
    let bay = bucket / d.days as usize;
    (set.len() as i64 - d.bays[bay].capacity).max(0)
}
