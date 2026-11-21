//! Task scheduling: place tasks on machines and employees, in order, under
//! skills, availability, time windows, dependencies and changeovers, while
//! keeping the schedule short and on time.

mod catalog;
pub mod constraints;
pub mod demo;
pub mod derived;
pub mod domain;
mod edits;
mod fill;
mod freeze;
mod kpis;
pub mod materials;
mod model;
mod names;
mod parameters;
pub mod prepare;
mod present;
pub mod schedule;
mod tables;
pub mod time;
mod view;
pub mod wire;

pub use domain::{Plan, Resource, Task};
pub use model::TaskScheduling;
