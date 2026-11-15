//! A furnace's loads in start order, and the changeovers between them.

use solverforge::stream::collector::{Accumulator, Collector};

use crate::derived::{Ctx, Derived};
use crate::domain::WorkOrder;

/// One scheduled order on a furnace: `(start, recipe, order, end)`.
pub type Row = (i64, usize, usize, i64);

/// The changeover a furnace makes into one load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changeover {
    /// When the furnace became free: the previous load's end, or 0 at the start.
    pub from: i64,
    /// Start of the load changed into.
    pub to: i64,
    /// Minutes the changeover needs.
    pub required: i64,
    /// Minutes missing between `from` and `to`.
    pub shortage: i64,
    /// Setpoints before and after, °C.
    pub temperatures: (i64, i64),
    /// An order of the load changed into.
    pub order: usize,
}

/// Changeovers on `furnace` for `rows` sorted by `(start, recipe, order)`.
/// Orders with the same start and recipe are one load; overlapping loads
/// have no changeover between them (the overlap rule covers them).
pub fn changeovers(d: &Derived, furnace: usize, rows: &[Row]) -> Vec<Changeover> {
    let mut out = Vec::new();
    // (end, temperature, process) of the furnace's state so far.
    let mut state = d.furnaces[furnace]
        .initial_temperature
        .map(|temperature| (0, temperature, None));
    let mut last_load: Option<(i64, usize)> = None;
    for &(start, recipe_idx, order, end) in rows {
        if last_load == Some((start, recipe_idx)) {
            continue;
        }
        last_load = Some((start, recipe_idx));
        let recipe = &d.recipes[recipe_idx];
        match state {
            Some((free, temperature, process)) if start >= free => {
                let required = d.changeover(furnace, (temperature, process), recipe);
                out.push(Changeover {
                    from: free,
                    to: start,
                    required,
                    shortage: (required - (start - free)).max(0),
                    temperatures: (temperature, recipe.temperature),
                    order,
                });
                state = Some((end, recipe.temperature, Some(recipe.process)));
            }
            Some((free, ..)) if end <= free => {}
            _ => state = Some((end, recipe.temperature, Some(recipe.process))),
        }
    }
    out
}

/// One furnace's scheduled orders, kept sorted as the solver moves them.
#[derive(Default)]
pub struct Timeline {
    ctx: Option<Ctx>,
    furnace: usize,
    rows: Vec<Row>,
}

impl Timeline {
    pub fn changeovers(&self) -> Vec<Changeover> {
        match &self.ctx {
            Some(ctx) => changeovers(ctx, self.furnace, &self.rows),
            None => Vec::new(),
        }
    }

    /// Minutes of changeover missing on the furnace.
    pub fn shortage(&self) -> i64 {
        self.changeovers().iter().map(|c| c.shortage).sum()
    }

    /// Minutes the furnace spends changing over.
    pub fn changeover_minutes(&self) -> i64 {
        self.changeovers().iter().map(|c| c.required).sum()
    }
}

/// Collects a furnace's scheduled orders into a [`Timeline`].
pub struct TimelineCollector;

impl<'i> Collector<&'i WorkOrder> for TimelineCollector {
    type Value = (Ctx, usize, Row);
    type Result = Timeline;
    type Accumulator = TimelineAccumulator;

    fn extract(&self, order: &'i WorkOrder) -> Self::Value {
        let placed = order.placed().expect("only scheduled orders are grouped");
        (
            order.ctx.clone(),
            placed.furnace,
            (placed.start, order.recipe_idx(), order.index, placed.end),
        )
    }

    fn create_accumulator(&self) -> TimelineAccumulator {
        TimelineAccumulator(Timeline::default())
    }
}

pub struct TimelineAccumulator(Timeline);

impl Accumulator<(Ctx, usize, Row), Timeline> for TimelineAccumulator {
    type Retraction = Row;

    fn accumulate(&mut self, (ctx, furnace, row): (Ctx, usize, Row)) -> Row {
        self.0.ctx.get_or_insert(ctx);
        self.0.furnace = furnace;
        let at = self.0.rows.partition_point(|r| *r < row);
        self.0.rows.insert(at, row);
        row
    }

    fn retract(&mut self, row: Row) {
        if let Ok(at) = self.0.rows.binary_search(&row) {
            self.0.rows.remove(at);
        }
    }

    fn with_result<T>(&self, f: impl FnOnce(&Timeline) -> T) -> T {
        f(&self.0)
    }

    fn reset(&mut self) {
        self.0.rows.clear();
    }
}
