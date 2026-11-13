use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Choice, Day, Employee, Shift, Whole, Window};
use crate::wire::{Alternatives, Contract, CostGroup, Schedule, ScheduleRules, TagType, Team};

/// An employee scheduling dataset and, once solved, its assignments.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub schedule: Schedule,
    /// Tag types, for rules that count distinct tags of a type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contracts: Vec<Contract>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub schedule_rules: ScheduleRules,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cost_groups: Vec<CostGroup>,
    /// Groups of alternative shifts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Alternatives>,
    /// Employees who work together.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub teams: Vec<Team>,
    #[problem_fact_collection]
    pub employees: Vec<Employee>,
    #[planning_entity_collection]
    pub shifts: Vec<Shift>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub days: Vec<Day>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub windows: Vec<Window>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub choices: Vec<Choice>,
    #[problem_fact_collection]
    #[serde(skip)]
    pub wholes: Vec<Whole>,
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
