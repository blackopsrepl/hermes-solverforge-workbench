//! Field service routing: assign visits to technicians' shifts and order
//! each shift's route, under skills, tags, time windows, breaks, visit
//! dependencies, multi-vehicle visits and shift limits, while keeping
//! travel and cost low.

mod catalog;
pub mod constraints;
pub mod demo;
pub mod derived;
pub mod domain;
mod fill;
mod freeze;
mod kpis;
mod model;
mod names;
mod parameters;
pub mod prepare;
mod present;
pub mod roads;
pub mod route;
pub mod time;
mod view;
pub mod wire;

pub use domain::{Plan, Shift, Visit};
pub use model::FieldServiceRouting;
