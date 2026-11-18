//! Pick-up and delivery routing: assign jobs (a pick-up and one or more
//! deliveries) to driver shifts and order each shift's stops, under vehicle
//! capacity, time windows, breaks, skills and driver requirements, while
//! keeping travel short.

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
pub use model::PickupDeliveryRouting;
