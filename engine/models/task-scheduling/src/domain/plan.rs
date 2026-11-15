use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Resource, Task};
use crate::derived::Facts;
use crate::wire::{Changeover, Parameters, ResourceTransition, TagDef, TypeDef};

/// A task scheduling dataset and, once solved, its schedule.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_types: Vec<TypeDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub machine_types: Vec<TypeDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changeovers: Vec<Changeover>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub resource_transitions: Vec<ResourceTransition>,
    #[serde(default)]
    pub parameters: Parameters,
    #[problem_fact_collection]
    pub tasks: Vec<Task>,
    #[planning_entity_collection]
    pub resources: Vec<Resource>,
    /// The tasks the solver may place: every task that is not pinned.
    #[planning_list_element_collection(owner = "resources")]
    #[serde(skip)]
    pub task_values: Vec<usize>,
    /// Facts derived from the dataset by `prepare`.
    #[serde(skip)]
    pub facts: Arc<Facts>,
    #[constraint_weights]
    #[serde(default)]
    pub weights: ConstraintWeights,
    #[planning_score]
    #[serde(default)]
    pub score: Option<HardMediumSoftScore>,
}

