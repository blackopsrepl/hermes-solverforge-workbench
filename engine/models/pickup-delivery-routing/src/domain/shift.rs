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
        construction_element_order_key = "visit_order"
    )]
    pub visits: Vec<usize>,
    /// The whole itinerary stays as the dataset gave it.
    #[planning_pin]
    pub pinned: bool,
    /// This is the bench, not a driver shift.
    pub bench: bool,
    /// Route totals, written by the shift's update.
    pub active: bool,
    pub start: i64,
    pub end_arrival: i64,
    pub last_departure: i64,
    pub travel: i64,
    pub distance: i64,
    pub unreachable_legs: i64,
    pub end_late: i64,
    pub end_soft_late: i64,
    pub last_late: i64,
    pub last_soft_late: i64,
}

impl Shift {
    pub fn new(index: usize, bench: bool) -> Self {
        Self {
            index,
            visits: Vec::new(),
            pinned: false,
            bench,
            active: false,
            start: 0,
            end_arrival: 0,
            last_departure: 0,
            travel: 0,
            distance: 0,
            unreachable_legs: 0,
            end_late: 0,
            end_soft_late: 0,
            last_late: 0,
            last_soft_late: 0,
        }
    }
}

/// The one shift a visit may move to, if any: a break's own shift, a
/// pinned stop's shift, or the shift already holding another stop of the
/// same job (the bench included).
///
/// Read against the current plan, so a job only changes shift whole: a
/// ruin move takes all its stops out, and the first one put back decides
/// where the others go.
pub fn visit_owner(plan: &super::Plan, visit: usize) -> Option<usize> {
    let fact = plan.facts.visits.get(visit)?;
    if let Some(owner) = fact.owner {
        return Some(owner);
    }
    if let Some((shift, _)) = fact.pin {
        return Some(shift);
    }
    let (job, _) = plan.facts.stop(visit)?;
    plan.facts.jobs[job]
        .stops
        .iter()
        .filter(|&&other| other != visit)
        .find_map(|&other| plan.visits[other].shift)
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
