//! Furnace heat treatment scheduling: put work orders into furnace loads
//! under recipe, capacity, changeover, downtime and due-date rules, give each
//! load's manual tasks a crew, and roster the crews.

mod catalog;
pub mod constraints;
pub mod demo;
pub mod derived;
pub mod domain;
pub mod eval;
mod fill;
mod freeze;
mod kpis;
pub mod labels;
mod model;
mod names;
mod params;
pub mod prepare;
mod present;
pub mod schedule;
mod search;
pub mod time;
mod view;
pub mod wire;

pub use domain::{Coverage, Furnace, Operator, Plan, RosterDay, WorkOrder};
pub use model::FurnaceHeatTreatment;
