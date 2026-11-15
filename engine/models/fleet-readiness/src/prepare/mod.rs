//! Dataset validation and derivation.

mod candidates;
mod checks;
mod plan;
mod tasks;

pub use candidates::{candidates, restrict_from};
pub use plan::{prepare, MAX_HORIZON_DAYS, MAX_LABOUR, MAX_PARTS, MAX_TASK_DAYS};
