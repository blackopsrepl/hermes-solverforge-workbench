//! Dataset validation and derivation.

mod candidates;
mod plan;

pub use candidates::{furnace_fits, start_fits};
pub use plan::{prepare, MAX_CYCLE, MAX_DAYS};
