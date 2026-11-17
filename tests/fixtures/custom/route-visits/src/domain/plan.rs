use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

// @solverforge:neutral-solution
// @solverforge:begin solution-imports
use super::Route;
use super::Visit;
// @solverforge:end solution-imports

/// The root planning solution.
///
/// Fresh projects start as a neutral shell. Add fact collections, planning
/// entity collections, and variable fields through the CLI as your domain
/// takes shape.
#[planning_solution(
    constraints = "crate::constraints::create_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
pub struct Plan {
    // @solverforge:begin solution-collections
    #[planning_entity_collection]
    pub visits: Vec<Visit>,
    #[planning_entity_collection]
    pub routes: Vec<Route>,
    // @solverforge:end solution-collections
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    #[rustfmt::skip]
    pub fn new(
        // @solverforge:begin solution-constructor-params
        visits: Vec<Visit>,
        routes: Vec<Route>,
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            // @solverforge:begin solution-constructor-init
            visits,
            routes,
            // @solverforge:end solution-constructor-init
            score: None,
        }
    }
}
