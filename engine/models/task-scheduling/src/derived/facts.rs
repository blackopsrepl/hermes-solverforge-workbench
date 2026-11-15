//! Resolved, indexed facts the schedule and the constraints read.

use super::Calendar;

/// One dependency of a task, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dep {
    /// The preceding task.
    pub task: usize,
    /// Offsets count from the preceding task's end (else its start).
    pub from_end: bool,
    pub min_offset: i64,
    pub max_offset: Option<i64>,
    pub same_resource: bool,
    pub no_intermediate: bool,
}

/// A task, resolved.
#[derive(Debug, Clone, Default)]
pub struct TaskFacts {
    /// Minutes on each resource, by resource index.
    pub duration: Vec<i64>,
    /// Resources the task may run on, by resource index; `None` is any.
    pub allowed: Option<Vec<bool>>,
    /// Required tag indexes.
    pub required_tags: Vec<usize>,
    pub min_start: Option<i64>,
    pub max_end: Option<i64>,
    pub ideal_end: Option<i64>,
    pub due: Option<i64>,
    pub priority: i64,
    pub task_type: Option<usize>,
    /// Pauses over unavailable spans rather than waiting for a long gap.
    pub interruptible: bool,
    pub deps: Vec<Dep>,
    /// Number of dependency hops above the task, for construction order.
    pub depth: i64,
}

/// A resource, resolved.
#[derive(Debug, Clone, Default)]
pub struct ResourceFacts {
    pub start: i64,
    pub machine: bool,
    pub machine_type: Option<usize>,
    pub initial_type: Option<usize>,
    pub calendar: Calendar,
    /// By tag index: `None` when the resource lacks the tag, else the
    /// half-open span it holds (`i64::MIN..i64::MAX` for always).
    pub tags: Vec<Option<(i64, i64)>>,
}

/// A hand-over of dependent tasks from one resource to another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Transition {
    pub reward: i64,
    pub forbidden: bool,
}

/// Everything `prepare` derives; shared by every clone of a plan.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub tasks: Vec<TaskFacts>,
    /// Machines and employees; the unscheduled queue is not here.
    pub resources: Vec<ResourceFacts>,
    /// Index of the unscheduled queue among the plan's resources.
    pub queue: usize,
    /// By task: `(dependent task, index of the dependency in its list)`.
    pub dependents: Vec<Vec<(usize, usize)>>,
    pub task_types: usize,
    /// Changeover minutes by `(machine type, from type, to type)`.
    pub changeover: Vec<Option<i64>>,
    /// Hand-overs by `from * resources + to`.
    pub transitions: Vec<Transition>,
    /// Hard cost of one broken structural link.
    pub broken_link: i64,
}

impl Facts {
    /// Changeover minutes on machine type `machine_type` from task type
    /// `from` to `to`, when the dataset gives one.
    pub fn changeover(&self, machine_type: usize, from: usize, to: usize) -> Option<i64> {
        let n = self.task_types;
        self.changeover
            .get((machine_type * n + from) * n + to)
            .copied()
            .flatten()
    }

    /// The hand-over from resource `from` to resource `to`.
    pub fn transition(&self, from: usize, to: usize) -> Transition {
        let n = self.resources.len();
        if from >= n || to >= n {
            return Transition::default();
        }
        self.transitions[from * n + to]
    }

    /// Construction order: shallow tasks first, then by priority, then
    /// longest first.
    pub fn construction_key(&self, task: usize) -> i64 {
        let Some(facts) = self.tasks.get(task) else {
            return i64::MAX;
        };
        let longest = facts
            .duration
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .min(99_999);
        facts.depth * 1_000_000_000 - facts.priority.min(9_999) * 100_000 - longest
    }
}
