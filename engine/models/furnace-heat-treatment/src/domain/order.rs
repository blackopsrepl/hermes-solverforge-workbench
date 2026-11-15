use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::derived::{Ctx, Placed, Task, Values};
use crate::wire::Priority;

/// A customer order: parts that go through one recipe in one furnace load,
/// plus the manual tasks around it.
#[planning_entity]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkOrder {
    /// Position in the plan's work order list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// Id of the recipe the parts need. Orders with the same recipe may share a load.
    pub recipe: String,
    pub load_kg: i64,
    /// Earliest start: when the parts arrive at the furnace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// When the parts must be out of the furnace.
    pub due: String,
    #[serde(default)]
    pub priority: Priority,
    /// Furnace id the order runs in, when scheduled.
    #[serde(default)]
    pub furnace: Option<String>,
    /// Charge time, `YYYY-MM-DDTHH:MM`, when scheduled.
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub load_build_operator: Option<String>,
    #[serde(default)]
    pub program_operator: Option<String>,
    #[serde(default)]
    pub quench_operator: Option<String>,
    #[serde(default)]
    pub unload_operator: Option<String>,
    /// Keep the furnace, start and crew as given; the solver will not change them.
    #[planning_pin]
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    #[serde(skip)]
    pub candidates: Values,
    #[serde(skip)]
    pub load_build_candidates: Values,
    #[serde(skip)]
    pub program_candidates: Values,
    #[serde(skip)]
    pub quench_candidates: Values,
    #[serde(skip)]
    pub unload_candidates: Values,
    /// Furnace and start slot together: `furnace * slots + slot`.
    #[planning_variable(value_range_provider = "candidates", allows_unassigned = true)]
    #[serde(skip)]
    pub slot: Option<usize>,
    #[planning_variable(value_range_provider = "load_build_candidates", allows_unassigned = true)]
    #[serde(skip)]
    pub load_build_op: Option<usize>,
    #[planning_variable(value_range_provider = "program_candidates", allows_unassigned = true)]
    #[serde(skip)]
    pub program_op: Option<usize>,
    #[planning_variable(value_range_provider = "quench_candidates", allows_unassigned = true)]
    #[serde(skip)]
    pub quench_op: Option<usize>,
    #[planning_variable(value_range_provider = "unload_candidates", allows_unassigned = true)]
    #[serde(skip)]
    pub unload_op: Option<usize>,
    #[serde(skip)]
    pub ctx: Ctx,
}

impl WorkOrder {
    /// Where and when the order runs, when scheduled.
    pub fn placed(&self) -> Option<Placed> {
        self.slot.map(|value| self.ctx.place(self.index, value))
    }

    pub fn furnace_idx(&self) -> Option<usize> {
        self.slot.map(|value| value / self.ctx.slots)
    }

    pub fn op(&self, task: Task) -> Option<usize> {
        match task {
            Task::LoadBuild => self.load_build_op,
            Task::Program => self.program_op,
            Task::Quench => self.quench_op,
            Task::Unload => self.unload_op,
        }
    }

    pub fn op_mut(&mut self, task: Task) -> &mut Option<usize> {
        match task {
            Task::LoadBuild => &mut self.load_build_op,
            Task::Program => &mut self.program_op,
            Task::Quench => &mut self.quench_op,
            Task::Unload => &mut self.unload_op,
        }
    }

    pub fn op_candidates(&self, task: Task) -> &[usize] {
        match task {
            Task::LoadBuild => &self.load_build_candidates,
            Task::Program => &self.program_candidates,
            Task::Quench => &self.quench_candidates,
            Task::Unload => &self.unload_candidates,
        }
    }

    /// Wire field of a task's operator.
    pub fn op_wire_mut(&mut self, task: Task) -> &mut Option<String> {
        match task {
            Task::LoadBuild => &mut self.load_build_operator,
            Task::Program => &mut self.program_operator,
            Task::Quench => &mut self.quench_operator,
            Task::Unload => &mut self.unload_operator,
        }
    }

    pub fn op_wire(&self, task: Task) -> Option<&str> {
        match task {
            Task::LoadBuild => self.load_build_operator.as_deref(),
            Task::Program => self.program_operator.as_deref(),
            Task::Quench => self.quench_operator.as_deref(),
            Task::Unload => self.unload_operator.as_deref(),
        }
    }

    /// The window of a task when the order is scheduled and has the task.
    pub fn task_window(&self, task: Task) -> Option<(i64, i64)> {
        let placed = self.placed()?;
        self.ctx.task_window(self.index, task, placed.start, placed.end)
    }

    /// Tasks the scheduled order needs.
    pub fn tasks(&self) -> impl Iterator<Item = Task> + '_ {
        Task::ALL
            .into_iter()
            .filter(|task| self.task_window(*task).is_some())
    }

    /// Recipe index; orders of one recipe may share a load.
    pub fn recipe_idx(&self) -> usize {
        self.ctx.orders[self.index].recipe
    }

    /// Whether two scheduled orders form one furnace load.
    pub fn same_load(&self, other: &WorkOrder) -> bool {
        self.slot.is_some() && self.slot == other.slot && self.recipe_idx() == other.recipe_idx()
    }
}
