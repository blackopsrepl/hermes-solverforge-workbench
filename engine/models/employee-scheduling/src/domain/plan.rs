use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Employee, Shift};
use crate::wire::{Contract, Schedule};

/// An employee scheduling dataset and, once solved, its assignments.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub schedule: Schedule,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contracts: Vec<Contract>,
    #[problem_fact_collection]
    pub employees: Vec<Employee>,
    #[planning_entity_collection]
    pub shifts: Vec<Shift>,
    #[constraint_weights]
    #[serde(default)]
    pub weights: ConstraintWeights,
    #[planning_score]
    #[serde(default)]
    pub score: Option<HardMediumSoftScore>,
}
