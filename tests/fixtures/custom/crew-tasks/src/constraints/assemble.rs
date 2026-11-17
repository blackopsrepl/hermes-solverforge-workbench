//! Assembles the project's constraint set.

use crate::domain::Plan;
use solverforge::prelude::*;

/// Builds the constraint set the solver scores every solution against.
pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    // @solverforge:begin constraint-calls
    (
        super::skill_match::constraint(),
        super::spread_hours::constraint(),
        super::assign_every_task::constraint(),
    )
    // @solverforge:end constraint-calls
}
