use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{BaySlot, Day, Task};
use crate::wire::{Bay, Crew, Horizon, Parameters, Part, ReadinessRule, Unit};

/// A fleet maintenance dataset and, once solved, where and when each task runs.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub horizon: Horizon,
    #[serde(default, skip_serializing_if = "is_default")]
    pub parameters: Parameters,
    pub units: Vec<Unit>,
    pub bays: Vec<Bay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub crews: Vec<Crew>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readiness: Vec<ReadinessRule>,
    #[planning_entity_collection]
    pub tasks: Vec<Task>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub slots: Vec<BaySlot>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub days: Vec<Day>,
    #[constraint_weights]
    #[serde(default)]
    pub weights: ConstraintWeights,
    #[planning_score]
    #[serde(default)]
    pub score: Option<HardMediumSoftScore>,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}
