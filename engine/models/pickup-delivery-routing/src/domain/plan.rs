use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Shift, Visit};
use crate::derived::Facts;
use crate::wire::{
    Driver, Job, Parameters, PlanningWindow, PriorityWeight, Traffic, TravelMatrix,
};

/// A pick-up and delivery dataset and, once solved, its route plan.
///
/// Each driver shift drives an ordered list of visits: the stops of the
/// jobs it serves and its own breaks. Each visit's slot is its cascading
/// shadow, computed from the slot before it on its shift, so a move
/// re-times only the visits after the change, up to the first whose slot
/// stays the same, and every rule reads the visit it blames.
#[planning_solution(
    constraints = "crate::constraints::define_constraints",
    solver_toml = "../../solver.toml"
)]
#[shadow_variable_updates(
    list_owner = "shifts",
    inverse_field = "shift",
    previous_field = "route_previous",
    next_field = "route_next",
    cascading_field = "slot",
    cascading_compute = "compute_slot"
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
    /// Busy periods of the day, when travel takes longer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub traffic: Vec<Traffic>,
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
    /// The cascading slot of visit `visit`.
    pub fn compute_slot(&self, visit: usize) -> Option<crate::route::Slot> {
        crate::route::slot(self, visit)
    }
}
