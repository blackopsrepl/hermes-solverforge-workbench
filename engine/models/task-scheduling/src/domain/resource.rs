use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::wire::{Assignment, ResourceKind, Span, TagRef};

/// A machine or an employee, with the tasks it runs in processing order.
///
/// The plan also holds one resource that is neither: the unscheduled
/// queue, holding every task no machine or employee runs. A list variable
/// assigns every element, so leaving a task unassigned means queueing it
/// there.
#[planning_entity]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resource {
    /// Position in the plan; the planning id analysis reports.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: ResourceKind,
    /// Local date-time the resource is first available.
    pub start: String,
    /// Machines only: what changeover times depend on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_type: Option<String>,
    /// Machines only: the task type the machine is set up for at its start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<TagRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<Span>,
    /// Pins every task placed here that starts before this local date-time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_tasks_until: Option<String>,
    /// Tasks placed here, in processing order, as the dataset gave them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<Assignment>,
    /// Pinned tasks, in order, ahead of the planned ones.
    #[serde(skip)]
    pub frozen: Vec<usize>,
    /// The tasks the solver orders here, after the frozen ones.
    #[planning_list_variable(
        element_collection = "task_values",
        construction_element_order_key = "task_construction_order"
    )]
    #[serde(skip)]
    pub sequence: Vec<usize>,
    /// This is the unscheduled queue, not a machine or an employee.
    #[serde(skip)]
    pub unscheduled: bool,
}

/// List construction places tasks with fewer predecessors first, so a task
/// is placed after the tasks it depends on; urgent and long tasks first
/// within one depth.
pub fn task_construction_order(plan: &super::Plan, task: usize) -> i64 {
    plan.facts.construction_key(task)
}
