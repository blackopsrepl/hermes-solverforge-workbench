//! The dataset's own vocabulary and plain records.

mod records;
mod vocab;

pub use records::{Downtime, Parameters, Recipe, Schedule};
pub use vocab::{FurnaceKind, Priority, Process, Role, ShiftKind, Skill};
