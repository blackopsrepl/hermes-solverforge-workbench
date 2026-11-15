use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Coverage, Furnace, Operator, RosterDay, WorkOrder};
use crate::wire::{Parameters, Recipe, Schedule};

/// A heat treatment shop's week: furnaces, recipes, crews and the work
/// orders to run, and once solved the furnace schedule and the roster.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml",
    scalar_groups = "scalar_groups"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub schedule: Schedule,
    #[serde(default, skip_serializing_if = "Parameters::is_default")]
    pub parameters: Parameters,
    pub recipes: Vec<Recipe>,
    #[problem_fact_collection]
    pub furnaces: Vec<Furnace>,
    #[problem_fact_collection]
    #[serde(default)]
    pub operators: Vec<Operator>,
    /// Operators each shift needs, by role.
    #[problem_fact_collection]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage: Vec<Coverage>,
    #[planning_entity_collection]
    pub work_orders: Vec<WorkOrder>,
    /// One row per operator and roster day; `prepare` adds missing rows.
    #[planning_entity_collection]
    #[serde(default)]
    pub roster: Vec<RosterDay>,
    #[constraint_weights]
    #[serde(default)]
    pub weights: ConstraintWeights,
    #[planning_score]
    #[serde(default)]
    pub score: Option<HardMediumSoftScore>,
}

/// Variables placement and crew moves change together.
pub(super) fn scalar_groups() -> Vec<ScalarGroup<Plan>> {
    use crate::search::{crew_candidates, op_variable, placement_candidates};
    let crew = || {
        crate::derived::Task::ALL
            .into_iter()
            .map(|task| Plan::work_orders().scalar(op_variable(task)))
            .chain([Plan::roster().scalar("shift_value")])
    };
    vec![
        ScalarGroup::candidates(
            "placement",
            std::iter::once(Plan::work_orders().scalar("slot")).chain(crew()).collect(),
            placement_candidates,
        ),
        ScalarGroup::candidates("crew", crew().collect(), crew_candidates),
    ]
}
