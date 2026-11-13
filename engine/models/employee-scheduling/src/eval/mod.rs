//! Scoring helpers over the solver's grouped rows.

mod economy;
mod members;
mod patterns;
mod roster;
mod runs;
mod schedule;
mod together;
mod windows;

pub use economy::{
    activation, alternatives_off, demand, fill_up, gained_tags_kept, replacement_tags_kept,
    share_gap, shifts_gained, shifts_lost, unfairness, unreplaced, Coverage,
};
pub use members::{
    ByChoice, ByDay, ByShare, ByWindow, Members, MembersAccumulator, MembersCollector, Row,
};
pub use patterns::{
    day_patterns, gaps, location_change_shortfall, multi_day_patterns, overlap_minutes, pairings,
    rotation_blocks, weekly_rotation,
};
pub use roster::{Roster, RosterAccumulator, RosterCollector};
pub use runs::{day_runs, near_day_off, shift_runs, DayRunScores};
pub use schedule::{concurrency_excess, window_deviation};
pub use together::{colleagues, teams, TeamScore};
pub use windows::{rules, window_penalty};
