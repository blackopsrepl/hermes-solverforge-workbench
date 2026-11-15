//! Fleet maintenance and readiness planning: put each unit's maintenance
//! tasks into workshop bays on days that respect due limits, bays, crews,
//! parts and precedence, while keeping enough of the fleet ready each day.

mod catalog;
pub mod constraints;
pub mod demo;
pub mod derived;
pub mod domain;
pub mod eval;
mod freeze;
mod kpis;
mod model;
mod names;
mod parameters;
pub mod prepare;
mod present;
mod status;
pub mod time;
mod view;
pub mod wire;

pub use domain::{BaySlot, Day, Plan, Task};
pub use model::FleetReadiness;
