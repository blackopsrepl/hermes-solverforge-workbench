use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A driver shift and the visits it drives, in order; or the bench.
///
/// A list variable assigns every element, so the plan holds one more
/// shift that drives nowhere: the bench, holding the stops no driver
/// serves.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Shift {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    pub index: usize,
    #[planning_list_variable(
        element_collection = "visits",
        element_owner_fn = "visit_owner",
        element_group_fn = "job_stops",
        construction_element_order_key = "visit_order"
    )]
    pub visits: Vec<usize>,
    /// The whole itinerary stays as the dataset gave it.
    #[planning_pin]
    pub pinned: bool,
    /// This is the bench, not a driver shift.
    pub bench: bool,
}

impl Shift {
    pub fn new(index: usize, bench: bool) -> Self {
        Self {
            index,
            visits: Vec::new(),
            pinned: false,
            bench,
        }
    }
}

/// The one shift a visit may move to, if any: a break's own shift or a
/// pinned stop's shift.
pub fn visit_owner(plan: &super::Plan, visit: usize) -> Option<usize> {
    let fact = plan.facts.visits.get(visit)?;
    fact.owner.or(fact.pin.map(|(shift, _)| shift))
}

/// A job's stops in service order: SolverForge keeps them on one shift and
/// moves, ruins and places them as one unit. A break has no group.
pub fn job_stops(plan: &super::Plan, visit: usize, members: &mut Vec<usize>) {
    if let Some((job, _)) = plan.facts.stop(visit) {
        members.extend_from_slice(&plan.facts.jobs[job].stops);
    }
}

/// List construction places jobs in dataset order, each job's stops in
/// service order so a delivery finds its pick-up already planned, and
/// breaks last, between the stops.
pub fn visit_order(plan: &super::Plan, visit: usize) -> i64 {
    match plan.facts.stop(visit) {
        Some((job, seq)) => (job * 16 + seq) as i64,
        None => i64::MAX / 2 + visit as i64,
    }
}
