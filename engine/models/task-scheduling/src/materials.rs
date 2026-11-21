//! Parts in stock: what tasks use up when they start and make when they end.
//!
//! The plan's `stock` lists the units of each part on hand from the start or
//! arriving later. A task takes the parts it `consumes` when it starts and
//! adds the parts it `produces` when it ends. A task that starts while the
//! stock no longer holds what it needs is short by the missing units.

use std::sync::Arc;

/// When each part's units arrive: per part, `(minute, units)`; units on hand
/// from the start arrive at `i64::MIN`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Stock {
    pub arrivals: Vec<Vec<(i64, i64)>>,
}

/// The parts one task uses up and makes, and the plan's stock.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MaterialUse {
    pub consumes: Vec<(usize, i64)>,
    pub produces: Vec<(usize, i64)>,
    pub stock: Arc<Stock>,
}

/// A placed task's start, end and parts.
pub type Placed = (i64, i64, Arc<MaterialUse>);

/// The units the placed tasks find missing when they start: the stock's
/// arrivals and the parts tasks made by then, less what tasks started before
/// used up. Parts that arrive or are made at a minute are there for a task
/// starting that minute.
pub fn shortage<'a>(tasks: impl Iterator<Item = &'a Placed>) -> i64 {
    let mut events: Vec<(i64, bool, usize, i64)> = Vec::new();
    let mut stock = None;
    for (start, end, use_) in tasks {
        stock.get_or_insert_with(|| Arc::clone(&use_.stock));
        events.extend(use_.produces.iter().map(|&(p, q)| (*end, false, p, q)));
        events.extend(use_.consumes.iter().map(|&(p, q)| (*start, true, p, q)));
    }
    let Some(stock) = stock else {
        return 0;
    };
    for (part, arrivals) in stock.arrivals.iter().enumerate() {
        events.extend(arrivals.iter().map(|&(at, q)| (at, false, part, q)));
    }
    events.sort_unstable();
    let mut level = vec![0i64; stock.arrivals.len()];
    let mut short = 0;
    for (_, takes, part, quantity) in events {
        if takes {
            let before = (-level[part]).max(0);
            level[part] -= quantity;
            short += (-level[part]).max(0) - before;
        } else {
            level[part] += quantity;
        }
    }
    short
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_(
        consumes: &[(usize, i64)],
        produces: &[(usize, i64)],
        stock: &Arc<Stock>,
    ) -> Arc<MaterialUse> {
        Arc::new(MaterialUse {
            consumes: consumes.to_vec(),
            produces: produces.to_vec(),
            stock: Arc::clone(stock),
        })
    }

    #[test]
    fn a_task_is_short_by_what_neither_stock_nor_earlier_tasks_provide() {
        let stock = Arc::new(Stock {
            arrivals: vec![vec![(i64::MIN, 2), (600, 5)]],
        });
        // Takes 3 at 480 with 2 on hand: one short.
        let early = [(480, 540, use_(&[(0, 3)], &[], &stock))];
        assert_eq!(shortage(early.iter()), 1);
        // At 600 the delivery is in.
        let late = [(600, 660, use_(&[(0, 3)], &[], &stock))];
        assert_eq!(shortage(late.iter()), 0);
    }

    #[test]
    fn what_a_task_makes_is_there_once_it_ends() {
        let stock = Arc::new(Stock {
            arrivals: vec![Vec::new()],
        });
        let make = use_(&[], &[(0, 4)], &stock);
        let take = use_(&[(0, 4)], &[], &stock);
        let after = [(0, 60, make.clone()), (60, 90, take.clone())];
        assert_eq!(shortage(after.iter()), 0);
        let before = [(0, 60, make), (30, 90, take)];
        assert_eq!(shortage(before.iter()), 4);
    }
}
