//! Employee scheduling: assign employees to shifts under skills,
//! availability, contracts, preferences, demand, cost and fairness.

mod catalog;
pub mod constraints;
pub mod demo;
pub mod derived;
pub mod domain;
mod edits;
pub mod eval;
mod freeze;
mod kpis;
mod model;
mod names;
pub mod prepare;
mod present;
pub mod time;
mod view;
pub mod wire;

pub use domain::{Employee, Plan, Shift};
pub use model::EmployeeScheduling;
