//! The model's constraints: seventeen rules, each an incremental stream
//! over the tasks and the slots SolverForge keeps current.

mod define;
mod erased;
mod rule;
mod tasks;

pub use define::define_constraints;
pub use erased::Constraints;
pub use rule::{Rule, RULES};
