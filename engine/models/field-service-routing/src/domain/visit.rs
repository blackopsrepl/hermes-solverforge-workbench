use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::constraints::Rule;
use crate::route::{At, Slot};
use crate::tools::ToolUse;
use std::sync::Arc;

/// A visit or a break, with where and when it is planned.
///
/// The static fields come from `prepare` (only what joins between visits
/// need); the rest are shadows SolverForge keeps current.
#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct Visit {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    pub index: usize,
    pub is_break: bool,
    /// Service seconds as given, for costs while it cannot be timed.
    pub service: i64,
    /// The multi-vehicle group it belongs to.
    pub group: Option<usize>,
    /// Visits it may not be planned with, and visits it depends on.
    pub exclusions: Vec<usize>,
    pub preceding: Vec<usize>,
    /// Some visit depends on it.
    pub has_dependents: bool,
    /// Its connected set of visits under exclusions, and under
    /// dependencies, named by the set's first visit.
    pub exclusion_set: usize,
    pub dependency_set: usize,
    /// What it costs while in no list, rule by rule.
    #[serde(skip)]
    pub missing: Vec<(Rule, i64)>,
    /// The shared tools it needs, for visits that need any.
    #[serde(skip)]
    pub tools: Option<Arc<ToolUse>>,

    /// The shift holding this element (generated inverse shadow).
    pub shift: Option<usize>,
    /// The elements before and after it on that shift (generated shadows).
    pub route_previous: Option<usize>,
    pub route_next: Option<usize>,
    /// Where and when it is planned, and what it costs (cascading shadow).
    #[serde(skip)]
    pub slot: Option<Slot>,
}

/// Where a visit is in the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// On a shift, timed.
    Planned,
    /// On a shift, but it cannot be timed: it waits on a visit after it.
    Untimed,
    /// On the bench: unplanned.
    Benched,
    /// Not in any list yet, while construction or a ruin move places it.
    Pending,
}

impl Visit {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            is_break: false,
            service: 0,
            group: None,
            exclusions: Vec::new(),
            preceding: Vec::new(),
            has_dependents: false,
            exclusion_set: index,
            dependency_set: index,
            missing: Vec::new(),
            tools: None,
            shift: None,
            route_previous: None,
            route_next: None,
            slot: None,
        }
    }

    pub fn status(&self) -> Status {
        match (self.shift, &self.slot) {
            (None, _) => Status::Pending,
            (Some(_), Some(slot)) if slot.benched => Status::Benched,
            (Some(_), Some(_)) => Status::Planned,
            (Some(_), None) => Status::Untimed,
        }
    }

    pub fn planned(&self) -> bool {
        self.status() == Status::Planned
    }

    /// The slot of an element timed on a shift.
    pub fn planned_slot(&self) -> Option<&Slot> {
        self.slot.as_ref().filter(|slot| !slot.benched)
    }

    /// The element as planned on a shift.
    pub fn at(&self) -> Option<&At> {
        self.planned_slot().map(|slot| &slot.at)
    }
}
