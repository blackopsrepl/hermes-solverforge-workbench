//! The constraint set the plan names in `#[planning_solution]`.

use super::TaskConstraints;

/// Every constraint of the model.
pub fn define_constraints() -> TaskConstraints {
    TaskConstraints::new()
}
