use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::route::{At, Slot};

/// A stop of a job or a break of a shift, with where and when it is
/// planned.
///
/// The static fields come from `prepare`; the rest are shadows SolverForge
/// keeps current.
#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct Visit {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    pub index: usize,
    /// The job of a stop; `None` for a break.
    pub job: Option<usize>,
    /// The job's stop served right before this one.
    pub previous: Option<usize>,
    /// A break of its shift: fixed in time or floating.
    pub fixed_break: bool,
    pub floating_break: bool,
    /// A fixed break taken at a place of its own.
    pub located: bool,
    /// Service seconds before any skill multiplier.
    pub duration: i64,
    /// Cost of leaving the stop unplanned: medium for mandatory jobs, soft
    /// for optional ones.
    pub unplanned_medium: i64,
    pub unplanned_soft: i64,
    /// SLA seconds missed when the stop is left unplanned.
    pub sla_unplanned: i64,
    /// A pinned stop's shift.
    pub pin_shift: Option<usize>,

    /// The shift holding this visit (generated inverse shadow).
    pub shift: Option<usize>,
    /// The visits before and after it on that shift (generated shadows).
    pub route_previous: Option<usize>,
    pub route_next: Option<usize>,
    /// Where and when it is planned, and what it costs (cascading shadow).
    #[serde(skip)]
    pub slot: Option<Slot>,
}

/// Where a visit is in the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// On a driver shift.
    Planned,
    /// On the bench: unplanned.
    Benched,
    /// Not in any list yet, while construction or a ruin move places it.
    Pending,
}

impl Visit {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            job: None,
            previous: None,
            fixed_break: false,
            floating_break: false,
            located: false,
            duration: 0,
            unplanned_medium: 0,
            unplanned_soft: 0,
            sla_unplanned: 0,
            pin_shift: None,
            shift: None,
            route_previous: None,
            route_next: None,
            slot: None,
        }
    }

    pub fn status(&self) -> Status {
        match (self.shift, &self.slot) {
            (Some(_), Some(slot)) if slot.benched => Status::Benched,
            (Some(_), Some(_)) => Status::Planned,
            _ => Status::Pending,
        }
    }

    /// The slot of a visit on a driver shift.
    pub fn planned_slot(&self) -> Option<&Slot> {
        self.slot.as_ref().filter(|slot| !slot.benched)
    }

    /// The visit as planned on a driver shift.
    pub fn at(&self) -> Option<&At> {
        self.planned_slot().map(|slot| &slot.at)
    }

    pub fn planned(&self) -> bool {
        self.status() == Status::Planned
    }

    /// Not on a driver shift: on the bench or still pending.
    pub fn unplanned(&self) -> bool {
        self.status() != Status::Planned
    }

    /// Service seconds as planned, or as given while unplanned.
    pub fn effective_duration(&self) -> i64 {
        self.at().map_or(self.duration, |at| at.end - at.start)
    }
}
