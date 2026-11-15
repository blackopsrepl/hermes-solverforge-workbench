//! The schedule a plan's task order implies: every task's resource, start,
//! end and changeover, derived the way a planner would time it.

mod timing;

pub use timing::{Changeover, Timing, UNASSIGNED};
