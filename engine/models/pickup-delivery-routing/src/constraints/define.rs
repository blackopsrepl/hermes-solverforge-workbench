use super::{jobs, shifts, visits, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new((
        visits::constraints(),
        jobs::constraints(),
        shifts::constraints(),
    ))
}
