//! Facts `prepare` derives from the dataset: ids resolved to indexes, times
//! to minutes, durations per resource, the changeover table.

mod calendar;
mod facts;

pub use calendar::Calendar;
pub use facts::{Dep, Facts, ResourceFacts, TaskFacts, Transition};
