use solverforge::prelude::*;

use super::{fit, per_employee, schedule_wide, staffing};
use crate::domain::Plan;

/// Every constraint of the model.
pub fn define_constraints() -> impl ConstraintSet<Plan, HardMediumSoftScore> {
    (
        staffing::constraints(),
        fit::constraints(),
        per_employee::constraints(),
        schedule_wide::constraints(),
    )
}
