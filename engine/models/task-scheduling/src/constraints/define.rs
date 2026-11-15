//! The constraint set the plan names in `#[planning_solution]`.

use super::{tasks, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new(tasks::constraints())
}
