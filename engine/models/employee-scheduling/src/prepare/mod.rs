//! Dataset validation and derivation.

mod builder;
mod plan;

pub use builder::Builder;
pub use plan::{prepare, MAX_SHIFT_DAYS};
