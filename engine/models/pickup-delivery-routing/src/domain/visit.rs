use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A stop of a job or a break of a shift, with where and when it is
/// planned.
///
/// The static fields come from `prepare`; the rest are shadows the owning
/// shift's update writes.
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
    /// Position in that shift's list (generated index shadow).
    pub position: Option<usize>,
    /// The holding shift is the bench.
    pub benched: bool,
    pub arrival: i64,
    pub start: i64,
    pub end: i64,
    /// Travel from the previous place, seconds and meters.
    pub travel: i64,
    pub distance: i64,
    pub wait: i64,
    pub unreachable: bool,
    pub over_capacity: i64,
    pub late_start: i64,
    pub late_end: i64,
    pub sla_late: i64,
    pub earliest_day: i64,
    pub pin_moved: i64,
    pub not_required: bool,
    pub prohibited: bool,
    pub not_preferred: bool,
    pub skills_missing: i64,
    pub tags_missing: i64,
    pub overqualified: i64,
    /// Breaks: seconds late to a fixed break, seconds a floating break
    /// ends late, and break seconds during jobs that forbid it.
    pub break_late: i64,
    pub break_during_job: i64,
    /// Rides past this job stop's place before serving it.
    pub loops: i64,
    /// On a job's first stop: seconds over its time burden, and seconds
    /// on board together with jobs it may not ride with.
    pub burden: i64,
    pub pooling: i64,
    pub prohibited_ride: i64,
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
            position: None,
            benched: false,
            arrival: 0,
            start: 0,
            end: 0,
            travel: 0,
            distance: 0,
            wait: 0,
            unreachable: false,
            over_capacity: 0,
            late_start: 0,
            late_end: 0,
            sla_late: 0,
            earliest_day: 0,
            pin_moved: 0,
            not_required: false,
            prohibited: false,
            not_preferred: false,
            skills_missing: 0,
            tags_missing: 0,
            overqualified: 0,
            break_late: 0,
            break_during_job: 0,
            loops: 0,
            burden: 0,
            pooling: 0,
            prohibited_ride: 0,
        }
    }

    pub fn status(&self) -> Status {
        match self.shift {
            None => Status::Pending,
            Some(_) if self.benched => Status::Benched,
            Some(_) => Status::Planned,
        }
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
        if self.planned() {
            self.end - self.start
        } else {
            self.duration
        }
    }
}
