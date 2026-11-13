//! Scoring helpers over the solver's grouped rows.

mod roster;
mod runs;
mod windows;

pub use roster::{Roster, RosterAccumulator, RosterCollector};
pub use runs::{day_runs, near_day_off, shift_runs, DayRunScores};
pub use windows::{rules, window_penalty};
