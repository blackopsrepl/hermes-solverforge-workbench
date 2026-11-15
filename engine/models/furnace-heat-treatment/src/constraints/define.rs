use super::{furnace, roster, tasks, timeline, timing, Constraints};

/// Every constraint of the model, behind the short [`Constraints`] type.
pub fn define_constraints() -> Constraints {
    Constraints::new((
        furnace::constraints(),
        timeline::constraints(),
        tasks::constraints(),
        roster::constraints(),
        timing::constraints(),
    ))
}
