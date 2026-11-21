use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Resource, Task};
use crate::derived::Facts;
use crate::schedule::Slot;
use crate::wire::{Changeover, Parameters, ResourceTransition, StockArrival, TagDef, TypeDef};

/// A task scheduling dataset and, once solved, its schedule.
///
/// Each resource runs an ordered list of tasks. A task's slot (where and
/// when it runs, and what each rule costs for it) follows from the task
/// before it on its resource and from the tasks it depends on, which other
/// resources may run; SolverForge keeps every slot current as tasks move.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[shadow_variable_updates(
    list_owner = "resources",
    inverse_field = "lane",
    previous_field = "previous",
    next_field = "next",
    cascading_field = "slot",
    cascading_compute = "compute_slot",
    cascading_dependencies = "slot_dependencies"
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
    /// Parts on hand from the start or arriving later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock: Vec<StockArrival>,
    #[problem_fact_collection]
    pub tasks: Vec<Task>,
    /// Machines and employees, then the unscheduled queue and the frozen
    /// lane.
    #[planning_entity_collection]
    pub resources: Vec<Resource>,
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

impl Plan {
    /// The slot of task `task`, from the task before it and the tasks it
    /// depends on.
    pub fn compute_slot(&self, task: usize) -> Option<Slot> {
        crate::schedule::compute(self, task)
    }

    /// The tasks task `task` depends on.
    pub fn slot_dependencies(&self, task: usize) -> impl Iterator<Item = usize> + '_ {
        self.facts
            .tasks
            .get(task)
            .into_iter()
            .flat_map(|facts| facts.deps.iter().map(|dep| dep.task))
    }
}
