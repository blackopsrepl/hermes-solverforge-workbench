//! The constraint set the plan names in `#[planning_solution]`.

use super::{links, routes, visits, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new((
        visits::constraints(),
        routes::constraints(),
        links::constraints(),
    ))
}
