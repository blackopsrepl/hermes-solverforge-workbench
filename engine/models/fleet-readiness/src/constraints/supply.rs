//! Spare parts against the stock that has arrived.

use solverforge::prelude::*;

use crate::domain::Plan;
use crate::eval::{shortage, ByPart, Ledger, LedgerCollector, Row};

type Score = HardMediumSoftScore;

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .project(ByPart)
        .group_by(|row: &Row| row.bucket as usize, LedgerCollector)
        .penalize(hard_weight(|part: &usize, ledger: &Ledger| {
            Score::of_hard(shortage(*part, ledger))
        }))
        .named("Parts short"),)
}
