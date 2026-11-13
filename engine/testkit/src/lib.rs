//! A three-constraint roster model and the engine binary that runs it.
//!
//! The host's contract tests and the plugin's tests drive this model, so they
//! exercise every engine command on a real SolverForge model without depending
//! on a stock model's dataset format.

pub mod toy;
