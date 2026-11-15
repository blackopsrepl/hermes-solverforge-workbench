use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Shift, Visit};
use crate::derived::Facts;
use crate::wire::{Driver, Job, Parameters, PlanningWindow, PriorityWeight, TravelMatrix};

/// A pick-up and delivery dataset and, once solved, its route plan.
///
/// Each driver shift drives an ordered list of visits: the stops of the
/// jobs it serves and its own breaks. A shift's update recomputes its
/// route (arrival, service, load) and writes it onto its visits, so every
/// rule reads planned times straight from the visit or shift it blames.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[shadow_variable_updates(
    list_owner = "shifts",
    inverse_field = "shift",
    index_field = "position",
    post_update_listener = "refresh_route"
)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub parameters: Parameters,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning_window: Option<PlanningWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub priority_weights: Vec<PriorityWeight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel_matrix: Option<TravelMatrix>,
    pub drivers: Vec<Driver>,
    pub jobs: Vec<Job>,
    /// Every stop of every job, then every break of every shift.
    #[problem_fact_collection]
    #[serde(skip)]
    pub visits: Vec<Visit>,
    /// Every driver shift, then the bench holding unplanned stops.
    #[planning_entity_collection]
    #[serde(skip)]
    pub shifts: Vec<Shift>,
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
    /// Recomputes shift `shift`'s route and writes it onto its visits.
    pub fn refresh_route(&mut self, shift: usize) {
        crate::route::refresh(self, shift);
    }
}
