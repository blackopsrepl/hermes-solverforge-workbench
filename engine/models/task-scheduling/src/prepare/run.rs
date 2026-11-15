//! `prepare`: check a dataset and derive what scoring and solving read.

use std::sync::Arc;

use solverforge_workbench_host::{Issue, Severity};

use super::facts::{derive, Index};
use super::place::{place, queue};
use crate::constraints::Rule;
use crate::derived::Facts;
use crate::domain::Plan;

/// Checks `plan`, derives its facts and places its tasks. On any error the
/// plan is left without facts and must not be scored.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    plan.resources.retain(|resource| !resource.unscheduled);
    for (index, task) in plan.tasks.iter_mut().enumerate() {
        task.index = index;
        task.pinned_start = None;
    }
    for (index, resource) in plan.resources.iter_mut().enumerate() {
        resource.index = index;
    }
    let mut issues = Vec::new();
    let (facts, placed) = {
        let index = Index::new(plan, &mut issues);
        let facts = derive(plan, &index, &mut issues);
        let placed = place(plan, &index, &mut issues);
        (facts, placed)
    };
    if issues.iter().any(|issue| issue.severity == Severity::Error) {
        return issues;
    }
    warn_unschedulable(plan, &facts, &mut issues);

    for (t, start) in placed.pinned_start.into_iter().enumerate() {
        plan.tasks[t].pinned_start = start;
    }
    for ((resource, frozen), sequence) in plan
        .resources
        .iter_mut()
        .zip(placed.frozen)
        .zip(placed.sequence)
    {
        for &t in &frozen {
            plan.tasks[t].pinned = true;
        }
        resource.frozen = frozen;
        resource.sequence = sequence;
    }
    for &t in &placed.queued {
        plan.tasks[t].pinned = false;
    }
    let queue_index = plan.resources.len();
    // Open tasks start on no list at all, so construction places every one
    // of them where it scores best: on a resource or in the queue.
    plan.resources.push(queue(queue_index));
    let frozen: Vec<bool> = {
        let mut frozen = vec![false; plan.tasks.len()];
        for resource in &plan.resources {
            for &t in &resource.frozen {
                frozen[t] = true;
            }
        }
        frozen
    };
    plan.task_values = (0..plan.tasks.len()).filter(|&t| !frozen[t]).collect();
    plan.facts = Arc::new(facts);
    // Idle time is off unless a weight turns it on.
    if plan.weights.get(Rule::IdleTime.name()).is_none() {
        plan.weights.set(Rule::IdleTime.name(), 0);
    }
    plan.score = None;
    issues
}

/// Warns about tasks no resource can run within their window.
fn warn_unschedulable(plan: &Plan, facts: &Facts, issues: &mut Vec<Issue>) {
    for (task, tf) in plan.tasks.iter().zip(&facts.tasks) {
        let fits: Vec<usize> = (0..facts.resources.len())
            .filter(|&r| tf.allowed.as_ref().is_none_or(|allowed| allowed[r]))
            .filter(|&r| {
                tf.required_tags
                    .iter()
                    .all(|&tag| facts.resources[r].tags[tag].is_some())
            })
            .collect();
        if fits.is_empty() {
            issues.push(Issue::warning(
                "no_eligible_resource",
                format!(
                    "no resource is allowed and carries every tag task {:?} requires",
                    task.id
                ),
            ));
            continue;
        }
        if let (Some(min_start), Some(max_end)) = (tf.min_start, tf.max_end) {
            let shortest = fits.iter().map(|&r| tf.duration[r]).min().unwrap_or(0);
            if max_end - min_start < shortest {
                issues.push(Issue::warning(
                    "impossible_window",
                    format!(
                        "task {:?} cannot fit between its earliest start and its deadline on any resource",
                        task.id
                    ),
                ));
            }
        }
    }
}
