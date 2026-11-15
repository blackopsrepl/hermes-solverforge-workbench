use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

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
}
