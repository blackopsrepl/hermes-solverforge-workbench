//! Scoring helpers over the solver's grouped rows.

mod members;
mod patterns;
mod roster;
mod runs;
mod schedule;
mod windows;

pub use members::{ByDay, ByWindow, Members, MembersAccumulator, MembersCollector, Row};
pub use patterns::{
    day_patterns, gaps, multi_day_patterns, overlap_minutes, pairings, rotation_blocks,
    weekly_rotation,
};
pub use roster::{Roster, RosterAccumulator, RosterCollector};
pub use runs::{day_runs, near_day_off, shift_runs, DayRunScores};
pub use schedule::{concurrency_excess, window_deviation};
pub use windows::{rules, window_penalty};
