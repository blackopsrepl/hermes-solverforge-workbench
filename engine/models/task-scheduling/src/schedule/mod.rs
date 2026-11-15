//! The schedule a plan's task order implies: every task's resource, start,
//! end and changeover, derived the way a planner would time it.

mod read;
mod slot;

pub use read::{Schedule, UNASSIGNED};
pub use slot::{begin, compute, frozen_end, run, Changeover, Slot};
