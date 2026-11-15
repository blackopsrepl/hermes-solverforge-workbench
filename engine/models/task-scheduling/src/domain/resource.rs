use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::wire::{Assignment, ResourceKind, Span, TagRef};

/// A machine or an employee, with the tasks it runs in processing order.
///
/// The plan also holds two lanes that are neither. The unscheduled queue
/// holds every task no machine or employee runs: a list variable assigns
/// every element, so leaving a task unassigned means queueing it there. The
/// frozen lane holds every frozen task and never changes; each frozen task
/// runs on the resource it was frozen on, ahead of that resource's planned
/// tasks.
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
    /// Frozen tasks run here, in order, ahead of the planned ones.
    #[serde(skip)]
    pub frozen: Vec<usize>,
    /// The tasks the solver orders here, after the frozen ones.
    #[planning_list_variable(
        element_collection = "tasks",
        construction_element_order_key = "task_construction_order"
    )]
    #[serde(skip)]
    pub sequence: Vec<usize>,
    #[serde(skip)]
    pub lane: Lane,
    /// The frozen lane: the solver keeps its tasks where they are.
    #[planning_pin]
    #[serde(skip)]
    pub pinned: bool,
}

/// What a resource of the plan is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Lane {
    /// A machine or an employee.
    #[default]
    Resource,
    /// The unscheduled queue.
    Queue,
    /// The frozen lane.
    Frozen,
}

impl Resource {
    /// An empty lane that is not a machine or an employee.
    pub fn lane(index: usize, lane: Lane, name: &str) -> Self {
        Self {
            index,
            id: String::new(),
            name: Some(name.to_string()),
            kind: Default::default(),
            start: String::new(),
            machine_type: None,
            initial_task_type: None,
            tags: Vec::new(),
            unavailable: Vec::new(),
            freeze_tasks_until: None,
            tasks: Vec::new(),
            frozen: Vec::new(),
            sequence: Vec::new(),
            lane,
            pinned: lane == Lane::Frozen,
        }
    }

    /// A machine or an employee.
    pub fn is_resource(&self) -> bool {
        self.lane == Lane::Resource
    }
}

/// List construction places tasks with fewer predecessors first, so a task
/// is placed after the tasks it depends on; urgent and long tasks first
/// within one depth.
pub fn task_construction_order(plan: &super::Plan, task: usize) -> i64 {
    plan.facts.construction_key(task)
}
