use super::{per_task, per_unit, readiness, supply, workshop, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new((
        per_task::constraints(),
        workshop::constraints(),
        per_unit::constraints(),
        supply::constraints(),
        readiness::constraints(),
    ))
}
