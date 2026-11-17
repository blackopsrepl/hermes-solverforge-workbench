//! Constraint definitions.
//!
//! Add constraint modules with `solverforge generate constraint ...`.
//! The neutral shell starts with an empty constraint set.

// @solverforge:begin constraint-modules
mod assign_every_task;
mod skill_match;
mod spread_hours;
// @solverforge:end constraint-modules

mod assemble;

pub use assemble::create_constraints;
