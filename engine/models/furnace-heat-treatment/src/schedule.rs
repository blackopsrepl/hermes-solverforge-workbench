//! The furnace schedule read off a plan: loads per furnace and the
//! changeovers between them, for KPIs, the view and fill.

use std::collections::BTreeMap;

use crate::domain::Plan;
use crate::eval::{changeovers, Changeover, Row};

/// One furnace load: the scheduled orders sharing a furnace, start and recipe.
#[derive(Debug, Clone)]
pub struct Load {
    pub furnace: usize,
    pub start: i64,
    pub end: i64,
    pub recipe: usize,
    pub orders: Vec<usize>,
    pub kg: i64,
}

/// Every load, by furnace and start.
pub fn loads(plan: &Plan) -> Vec<Load> {
    let mut by_key: BTreeMap<(usize, i64, usize), Load> = BTreeMap::new();
    for order in &plan.work_orders {
        let Some(p) = order.placed() else { continue };
        let load = by_key
            .entry((p.furnace, p.start, order.recipe_idx()))
            .or_insert_with(|| Load {
                furnace: p.furnace,
                start: p.start,
                end: p.end,
                recipe: order.recipe_idx(),
                orders: Vec::new(),
                kg: 0,
            });
        load.orders.push(order.index);
        load.kg += order.load_kg;
    }
    by_key.into_values().collect()
}

/// The changeovers of one furnace.
pub fn furnace_changeovers(plan: &Plan, furnace: usize) -> Vec<Changeover> {
    let Some(ctx) = plan.work_orders.first().map(|o| o.ctx.clone()) else {
        return Vec::new();
    };
    let mut rows: Vec<Row> = plan
        .work_orders
        .iter()
        .filter_map(|o| {
            let p = o.placed()?;
            (p.furnace == furnace).then_some((p.start, o.recipe_idx(), o.index, p.end))
        })
        .collect();
    rows.sort_unstable();
    changeovers(&ctx, furnace, &rows)
}
