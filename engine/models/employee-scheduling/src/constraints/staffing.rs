//! Shifts the plan leaves open.

use solverforge::prelude::*;

use crate::domain::{Plan, Shift};

type Score = HardMediumSoftScore;

fn open_in_horizon(shift: &Shift) -> bool {
    let info = shift.ctx.shift(shift.index);
    // Alternatives are governed by their group's count instead.
    shift.employee_idx.is_none() && info.in_horizon && info.alternative.is_none()
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    let f = ConstraintFactory::<Plan, Score>::new;
    (
        f().for_each(Plan::shifts())
            .filter(|shift: &Shift| {
                open_in_horizon(shift) && !shift.ctx.shift(shift.index).optional
            })
            .penalize(|shift: &Shift| Score::of(0, shift.ctx.shift(shift.index).importance, 0))
            .named("Mandatory shift left open"),
        f().for_each(Plan::shifts())
            .filter(|shift: &Shift| open_in_horizon(shift) && shift.ctx.shift(shift.index).optional)
            .penalize(|shift: &Shift| Score::of_soft(shift.ctx.shift(shift.index).importance))
            .named("Optional shift left open"),
    )
}
