use super::{fit, per_employee, replanning, schedule_wide, staffing, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new((
        staffing::constraints(),
        fit::constraints(),
        per_employee::constraints(),
        schedule_wide::constraints(),
        replanning::constraints(),
    ))
}
