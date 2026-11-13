//! The model's constraints, grouped by what they look at.

mod assignment;
mod define;
mod fit;
mod per_employee;
mod replanning;
mod schedule_wide;
mod staffing;

pub use define::define_constraints;
