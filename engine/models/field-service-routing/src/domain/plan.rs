use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::{Shift, Visit};
use crate::derived::Facts;
use crate::route::Slot;
use crate::wire::{
    Parameters, PlanningWindow, PriorityWeight, Tool, Traffic, TravelMatrix, Vehicle, VisitGroup,
};

/// A field service dataset and, once solved, its route plan.
///
/// Each vehicle shift (and each linked shift) drives an ordered list of
/// visits and its own breaks. Each visit's slot is its cascading shadow,
/// computed from the slot before it on its shift, from the visits it
/// depends on, and together with the other visits of its multi-vehicle
/// group, so a move re-times only the visits after the change.
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
    cascading_compute = "compute_slot",
    cascading_dependencies = "slot_dependencies",
    cascading_peers = "slot_peers"
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
    /// Skill and tag names the dataset declares, checked when given.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Equipment technicians share, one visit at a time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    pub vehicles: Vec<Vehicle>,
    #[serde(default)]
    pub visits: Vec<crate::wire::Visit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub visit_groups: Vec<VisitGroup>,
    /// Every visit (group members included), then every break.
    #[problem_fact_collection]
    #[serde(skip)]
    pub elements: Vec<Visit>,
    /// Every vehicle shift, every linked shift, then the bench holding
    /// unplanned visits.
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
    /// The cascading slot of element `element`.
    pub fn compute_slot(&self, element: usize) -> Option<Slot> {
        crate::route::slot(self, element)
    }

    /// The visits element `element` waits on.
    pub fn slot_dependencies(&self, element: usize) -> impl Iterator<Item = usize> + '_ {
        self.facts
            .visits
            .get(element)
            .into_iter()
            .flat_map(|fact| fact.dependencies.iter().map(|d| d.preceding))
    }

    /// The other visits of element `element`'s multi-vehicle group.
    pub fn slot_peers(&self, element: usize) -> impl Iterator<Item = usize> + '_ {
        self.facts
            .visits
            .get(element)
            .and_then(|fact| fact.group)
            .into_iter()
            .flat_map(move |g| self.facts.groups[g].visits.iter().copied())
            .filter(move |&v| v != element)
    }
}
