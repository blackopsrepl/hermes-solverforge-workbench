use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A vehicle shift or a linked shift and the visits it drives, in order;
/// or the bench.
///
/// A list variable assigns every element, so the plan holds one more
/// shift that drives nowhere: the bench, holding the visits no shift
/// serves.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Shift {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    pub index: usize,
    #[planning_list_variable(
        element_collection = "elements",
        element_owner_fn = "element_owner",
        element_group_fn = "shift_chain",
        construction_element_order_key = "element_order"
    )]
    pub visits: Vec<usize>,
    /// The whole itinerary stays as the dataset gave it.
    #[planning_pin]
    pub pinned: bool,
    /// This is the bench, not a shift.
    pub bench: bool,
    /// The vehicle driving it.
    pub vehicle: usize,
    /// A linked shift: its component shifts.
    pub components: Vec<usize>,
    /// Elements from here on are breaks.
    pub first_break: usize,
}

impl Shift {
    pub fn new(
        index: usize,
        bench: bool,
        vehicle: usize,
        components: Vec<usize>,
        first_break: usize,
    ) -> Self {
        Self {
            index,
            visits: Vec::new(),
            pinned: false,
            bench,
            vehicle,
            components,
            first_break,
        }
    }

    /// The shift serves at least one visit (not only breaks).
    pub fn serves(&self) -> bool {
        !self.bench && self.visits.iter().any(|&v| v < self.first_break)
    }

    /// How many visits (not breaks) it serves.
    pub fn served(&self) -> usize {
        self.visits.iter().filter(|&&v| v < self.first_break).count()
    }
}

/// The one shift an element may move to, if any: a break's own shift or a
/// pinned visit's shift.
pub fn element_owner(plan: &super::Plan, element: usize) -> Option<usize> {
    let fact = plan.facts.visits.get(element)?;
    fact.owner.or(fact.pin.map(|(shift, _)| shift))
}

/// A visit's same-shift dependency chain in service order: SolverForge
/// keeps it on one shift and moves, ruins and places it as one unit. A
/// break, or a visit in no chain, has no group.
pub fn shift_chain(plan: &super::Plan, element: usize, members: &mut Vec<usize>) {
    if let Some(c) = plan.facts.visits.get(element).and_then(|v| v.chain) {
        members.extend_from_slice(&plan.facts.chains[c]);
    }
}

/// List construction's order, worked out by `prepare`: breaks go first, so
/// every visit is inserted around its shift's breaks and pays for pushing one
/// late; placed last, a break only fits wherever the visits left room.
pub fn element_order(plan: &super::Plan, element: usize) -> i64 {
    plan.facts
        .visits
        .get(element)
        .map_or(i64::MIN / 2 + element as i64, |fact| fact.order)
}
