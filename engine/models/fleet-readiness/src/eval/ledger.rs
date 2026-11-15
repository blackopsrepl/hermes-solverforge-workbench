//! The use of one part over the horizon.

use solverforge::stream::collector::{Accumulator, Collector};

use super::Row;
use crate::derived::Ctx;

/// The tasks consuming a part, as `(start day, task, quantity)` in start order.
pub struct Ledger {
    ctx: Option<Ctx>,
    rows: Vec<(i64, u32, i64)>,
}

/// Collects part rows into a [`Ledger`].
pub struct LedgerCollector;

impl<'i> Collector<&'i Row> for LedgerCollector {
    type Value = (Ctx, (i64, u32, i64));
    type Result = Ledger;
    type Accumulator = LedgerAccumulator;

    fn extract(&self, row: &'i Row) -> (Ctx, (i64, u32, i64)) {
        (row.ctx.clone(), (row.day, row.task, row.value))
    }

    fn create_accumulator(&self) -> LedgerAccumulator {
        LedgerAccumulator(Ledger {
            ctx: None,
            rows: Vec::new(),
        })
    }
}

pub struct LedgerAccumulator(Ledger);

impl Accumulator<(Ctx, (i64, u32, i64)), Ledger> for LedgerAccumulator {
    type Retraction = (i64, u32, i64);

    fn accumulate(&mut self, (ctx, entry): (Ctx, (i64, u32, i64))) -> (i64, u32, i64) {
        self.0.ctx.get_or_insert(ctx);
        let at = self.0.rows.partition_point(|row| *row < entry);
        self.0.rows.insert(at, entry);
        entry
    }

    fn retract(&mut self, entry: (i64, u32, i64)) {
        if let Ok(at) = self.0.rows.binary_search(&entry) {
            self.0.rows.remove(at);
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&Ledger) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.rows.clear();
    }
}

/// Parts of kind `part` missing when the tasks need them: in start order,
/// each task takes what has arrived and is not yet used; what it cannot take
/// is short.
pub fn shortage(part: usize, ledger: &Ledger) -> i64 {
    let Some(d) = ledger.ctx.as_deref() else {
        return 0;
    };
    let info = &d.parts[part];
    let (mut used, mut short) = (0, 0);
    for &(day, _, quantity) in &ledger.rows {
        let available = (info.supplied_by(day) - used).max(0);
        let take = quantity.min(available);
        used += take;
        short += quantity - take;
    }
    short
}
