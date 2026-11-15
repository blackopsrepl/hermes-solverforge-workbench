//! Assignments from the dataset placed on the resources: pinned tasks into
//! each resource's frozen prefix, the rest into its planned sequence, and
//! every unplaced task into the unscheduled queue.

use solverforge_workbench_host::Issue;

use super::facts::{time, Index};
use crate::domain::{Plan, Resource};

/// What placing the dataset's assignments decided, per resource.
pub struct Placed {
    pub frozen: Vec<Vec<usize>>,
    pub sequence: Vec<Vec<usize>>,
    pub queued: Vec<usize>,
    pub pinned_start: Vec<Option<i64>>,
}

pub fn place(plan: &Plan, index: &Index, issues: &mut Vec<Issue>) -> Placed {
    let n = plan.tasks.len();
    let mut placed_on: Vec<Option<usize>> = vec![None; n];
    let mut pinned_start = vec![None; n];
    let mut frozen = Vec::with_capacity(plan.resources.len());
    let mut sequence = Vec::with_capacity(plan.resources.len());
    for (r, resource) in plan.resources.iter().enumerate() {
        let who = format!("resource {:?}", resource.id);
        let freeze = time(
            resource.freeze_tasks_until.as_deref(),
            &format!("freezeTasksUntil of {who}"),
            issues,
        );
        let mut pins = Vec::new();
        let mut planned = Vec::new();
        for assignment in &resource.tasks {
            let Some(t) = index.tasks.find(&assignment.task, &who, issues) else {
                continue;
            };
            if let Some(other) = placed_on[t] {
                issues.push(Issue::error(
                    "assigned_twice",
                    format!(
                        "task {:?} is placed on both {:?} and {:?}",
                        assignment.task, plan.resources[other].id, resource.id
                    ),
                ));
                continue;
            }
            placed_on[t] = Some(r);
            let start = time(
                assignment.start.as_deref(),
                &format!("the start of task {:?} on {who}", assignment.task),
                issues,
            );
            let frozen_by_time = matches!((start, freeze), (Some(s), Some(f)) if s < f);
            if plan.tasks[t].pinned || frozen_by_time {
                if !planned.is_empty() {
                    issues.push(Issue::error(
                        "pin_after_unpinned",
                        format!(
                            "task {:?} on {who} is pinned but follows a task that is not; pin the tasks before it too",
                            assignment.task
                        ),
                    ));
                    continue;
                }
                pins.push(t);
                pinned_start[t] = start;
            } else {
                planned.push(t);
            }
        }
        frozen.push(pins);
        sequence.push(planned);
    }
    let mut queued = Vec::new();
    for (t, task) in plan.tasks.iter().enumerate() {
        if placed_on[t].is_none() {
            if task.pinned {
                issues.push(Issue::warning(
                    "pinned_unassigned",
                    format!("task {:?} is pinned but on no resource; it stays unassigned and may be planned", task.id),
                ));
            }
            queued.push(t);
        }
    }
    Placed {
        frozen,
        sequence,
        queued,
        pinned_start,
    }
}

/// The unscheduled queue resource, empty.
pub fn queue(index: usize) -> Resource {
    Resource {
        index,
        id: String::new(),
        name: Some("Unscheduled".to_string()),
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
        unscheduled: true,
    }
}
