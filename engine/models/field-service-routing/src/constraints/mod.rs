//! The model's constraints, grouped by what they look at.

mod define;
mod erased;
mod links;
mod routes;
mod rules;
mod visits;

pub use define::define_constraints;
pub use erased::Constraints;
pub use rules::*;
