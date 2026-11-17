use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

// @solverforge:neutral-solution
// @solverforge:begin solution-imports
use super::Resource;
use super::Task;
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
    #[problem_fact_collection]
    pub resources: Vec<Resource>,
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    // @solverforge:end solution-collections
    #[constraint_weights]
    #[serde(default)]
    pub constraint_weights: ConstraintWeights,
    #[planning_score]
    pub score: Option<HardSoftScore>,
}

impl Plan {
    #[rustfmt::skip]
    pub fn new(
        // @solverforge:begin solution-constructor-params
        resources: Vec<Resource>,
        tasks: Vec<Task>,
        // @solverforge:end solution-constructor-params
    ) -> Self {
        Self {
            // @solverforge:begin solution-constructor-init
            resources,
            tasks,
            // @solverforge:end solution-constructor-init
            constraint_weights: ConstraintWeights::default(),
            score: None,
        }
    }
}
