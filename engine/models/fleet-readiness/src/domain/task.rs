use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use super::Plan;
use crate::derived::Ctx;
use crate::wire::{Labour, PartNeed, Placement};

/// A maintenance task: one unit in one bay for a run of whole days.
#[planning_entity]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Position in the plan's task list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    /// Id of the unit the work is done on.
    pub unit: String,
    /// What the work is (`A-check`, `Landing gear overhaul`…).
    pub name: String,
    /// Whole days the unit spends in the bay.
    pub days: i64,
    /// First day the work may start, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earliest_start: Option<String>,
    /// Last day by which the work must be finished, `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// Usage counter value (whole hours, cycles or kilometres) at which the work must have started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_usage: Option<i64>,
    /// Bay classes the work can be done in; any class when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bay_classes: Vec<String>,
    /// Technicians needed on each day of the work.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labour: Vec<Labour>,
    /// Parts consumed on the first day.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartNeed>,
    /// Ids of tasks of the same unit that must finish before this one starts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub after: Vec<String>,
    /// The work may wait beyond the horizon at a soft cost.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deferrable: bool,
    /// Days the unit stays out of service after the work (test flight,
    /// paperwork); the model parameter when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_to_service: Option<i64>,
    /// Bay and day the task was published with before replanning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<Placement>,
    /// Assigned bay id, if any.
    #[serde(default)]
    pub bay: Option<String>,
    /// Assigned first day, `YYYY-MM-DD`, if any.
    #[serde(default)]
    pub start: Option<String>,
    /// Keep `bay` and `start` as given; the solver will not change them.
    #[planning_pin]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// The assigned bay and first day as an index into the plan's slots.
    #[planning_variable(
        value_range_provider = "slots",
        allows_unassigned = true,
        candidate_values = "slot_candidates",
        nearby_value_candidates = "slot_candidates",
        nearby_value_distance_meter = "slot_distance"
    )]
    #[serde(skip)]
    pub slot_idx: Option<usize>,
    /// Slots the solver tries: suitable bays on days inside the window.
    #[serde(skip)]
    pub candidates: Vec<usize>,
    #[serde(skip)]
    pub ctx: Ctx,
}

impl Task {
    /// An open task with only an id, its unit, a name and a length.
    pub fn new(
        id: impl Into<String>,
        unit: impl Into<String>,
        name: impl Into<String>,
        days: i64,
    ) -> Self {
        Self {
            index: 0,
            id: id.into(),
            unit: unit.into(),
            name: name.into(),
            days,
            earliest_start: None,
            due_date: None,
            due_usage: None,
            bay_classes: Vec::new(),
            labour: Vec::new(),
            parts: Vec::new(),
            after: Vec::new(),
            deferrable: false,
            return_to_service: None,
            published: None,
            bay: None,
            start: None,
            pinned: false,
            slot_idx: None,
            candidates: Vec::new(),
            ctx: Ctx::default(),
        }
    }
}

pub(super) fn slot_candidates(plan: &Plan, entity_index: usize, _variable_index: usize) -> &[usize] {
    plan.tasks
        .get(entity_index)
        .map_or(&[], |task| task.candidates.as_slice())
}

/// How far a slot is from the task's current one (or, when open, from its
/// earliest start in the same bay): days apart plus a day per bay change.
pub(super) fn slot_distance(_plan: &Plan, task: &Task, slot: usize) -> f64 {
    let ctx = &task.ctx;
    let info = ctx.task(task.index);
    let (bay, day) = (ctx.slot_bay(slot), ctx.slot_day(slot));
    let (from_bay, from_day) = match task.slot_idx {
        Some(current) => (ctx.slot_bay(current), ctx.slot_day(current)),
        None => (bay, info.earliest),
    };
    ((day - from_day).abs() + i64::from(bay != from_bay)) as f64
}
