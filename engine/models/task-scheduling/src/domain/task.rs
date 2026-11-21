use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use std::sync::Arc;

use solverforge_workbench_host::stock::Part;

use crate::materials::MaterialUse;
use crate::schedule::Slot;
use crate::wire::{Dependency, DurationOn};

/// A task to run on one machine or by one employee.
#[problem_fact]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Default duration in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    /// Durations on specific resources, overriding the default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub duration_on: Vec<DurationOn>,
    /// Tags the resource must carry, all of them, while the task runs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tags: Vec<String>,
    /// Resources the task may run on; empty means any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    /// Parts the task takes from stock when it starts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumes: Vec<Part>,
    /// Parts the task adds to stock when it ends.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub produces: Vec<Part>,
    /// Earliest start (material arrives, upstream hand-off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_start: Option<String>,
    /// Latest end, a hard deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_end: Option<String>,
    /// Preferred end; finishing early or late both cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ideal_end: Option<String>,
    /// Due date; finishing later is lateness, but the task is still planned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    /// At least 1; the parameter `defaultPriority` when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// Run without pausing over unavailable spans; the parameter
    /// `interruptible` decides when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_interruptions: Option<bool>,
    /// Keep the task on its resource, at its position and start.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    /// The start a pinned task keeps, in minutes; set by `prepare` and `freeze`.
    #[serde(skip)]
    pub pinned_start: Option<i64>,
    /// Frozen: the resource the task runs on, the frozen task before it
    /// there and its rank among that resource's tasks.
    #[serde(skip)]
    pub frozen_on: Option<usize>,
    #[serde(skip)]
    pub frozen_previous: Option<usize>,
    #[serde(skip)]
    pub frozen_rank: usize,
    /// The last frozen task of its resource.
    #[serde(skip)]
    pub frozen_last: bool,
    /// What leaving the task in no list at all costs: its priority.
    #[serde(skip)]
    pub open_cost: i64,
    /// What the task costs on a resource where it cannot be timed: the
    /// broken-link penalty.
    #[serde(skip)]
    pub link_cost: i64,
    /// The parts it uses up and makes, with the plan's stock; set by
    /// `prepare` on tasks that use or make parts.
    #[serde(skip)]
    pub materials: Option<Arc<MaterialUse>>,

    /// The lane holding the task (list shadow).
    #[serde(skip)]
    pub lane: Option<usize>,
    /// The task before and after it in that lane (list shadows).
    #[serde(skip)]
    pub previous: Option<usize>,
    #[serde(skip)]
    pub next: Option<usize>,
    /// Where and when it runs (cascading list shadow).
    #[serde(skip)]
    pub slot: Option<Slot>,
}
