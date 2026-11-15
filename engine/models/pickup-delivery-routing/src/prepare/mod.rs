//! `prepare`: check a dataset, derive its facts, and lay its itineraries
//! into the shifts' lists.

mod build;
mod place;
mod run;

pub use run::prepare;
