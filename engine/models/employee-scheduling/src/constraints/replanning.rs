//! Shifts that leave the employee they were published to.

use solverforge::prelude::*;

use crate::domain::{Plan, Shift};

type Score = HardMediumSoftScore;

fn moved(shift: &Shift) -> bool {
    let info = shift.ctx.shift(shift.index);
    info.in_horizon
        && info
            .published
            .is_some_and(|e| shift.employee_idx != Some(e as usize))
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::shifts())
        .filter(moved)
        .penalize(|_: &Shift| Score::of_soft(1))
        .named("Published assignment changed"),)
}
