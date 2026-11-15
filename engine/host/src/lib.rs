//! Runs SolverForge models behind the workbench engine contract.
//!
//! A request arrives as one JSON line on stdin; events leave as JSON Lines on
//! stdout. Stock models and solverforge-cli projects both implement [`Model`]
//! and are listed with [`entry`] in the engine binary.

mod analysis;
pub mod fill;
mod inspect;
mod model;
mod output;
mod plan;
mod protocol;
mod run;
mod solve;
pub mod view;

pub use analysis::{Analysis, ConstraintReport, EntityReport, MatchReport};
pub use model::{
    ConstraintInfo, EntityName, Issue, Kpi, Level, Model, ParameterInfo, ScoreOf, Severity,
};
pub use protocol::{Command, EngineError, Request, Termination};
pub use run::{entry, run, run_io, Entry};
