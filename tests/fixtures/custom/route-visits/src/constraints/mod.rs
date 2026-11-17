//! Constraint definitions.
//!
//! Add constraint modules with `solverforge generate constraint ...`.
//! The neutral shell starts with an empty constraint set.

// @solverforge:begin constraint-modules
mod route_capacity;
mod short_routes;
// @solverforge:end constraint-modules

mod assemble;

pub use assemble::create_constraints;
