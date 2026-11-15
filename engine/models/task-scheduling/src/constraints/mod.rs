//! The model's constraints: one rule set scored over the derived schedule.
//!
//! A task's start depends on the tasks before it on its resource and on the
//! tasks it depends on, wherever those run, so moving one task re-times
//! tasks on other resources. Every rule reads that one timing, which the
//! set recomputes once per change and shares across all seventeen rules.

mod define;
mod rule;
mod rules;
mod set;

pub use define::define_constraints;
pub use rule::{Rule, RULES};
pub use rules::{evaluate, Blame, Sink};
pub use set::TaskConstraints;
