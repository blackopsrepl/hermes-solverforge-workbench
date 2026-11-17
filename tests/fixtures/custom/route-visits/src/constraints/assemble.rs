//! Assembles the project's constraint set.

use crate::domain::Plan;
use solverforge::prelude::*;

/// Builds the constraint set the solver scores every solution against.
pub fn create_constraints() -> impl ConstraintSet<Plan, HardSoftScore> {
    // @solverforge:begin constraint-calls
    (
        super::route_capacity::constraint(),
        super::short_routes::constraint(),
    )
    // @solverforge:end constraint-calls
}
