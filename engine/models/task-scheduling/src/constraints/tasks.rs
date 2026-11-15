//! The seventeen rules as incremental streams over tasks.
//!
//! Every per-task rule reads only the task's slot, so a move rescores
//! exactly the tasks whose slots it changed. A task without a slot is not
//! scheduled and costs its priority as unassigned: it is in no list, or it
//! cannot be timed because it waits on a task queued after it (or follows
//! one that does). Makespan is the longest span among each resource's last
//! planned task and last frozen task.

use solverforge::prelude::*;

use super::Rule;
use crate::domain::{Plan, Task};
use crate::schedule::Slot;

pub type Score = HardMediumSoftScore;

/// What task `task` costs under per-task rule `rule`.
fn amount(task: &Task, rule: Rule) -> i64 {
    match &task.slot {
        Some(slot) => slot.amount(rule),
        None if rule == Rule::Unassigned => task.open_cost,
        None => 0,
    }
}

fn hard(rule: Rule) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .filter(move |task: &Task| amount(task, rule) != 0)
        .penalize(hard_weight(move |task: &Task| {
            Score::of_hard(amount(task, rule))
        }))
        .named(rule.name())
}

fn medium(rule: Rule) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .filter(move |task: &Task| amount(task, rule) != 0)
        .penalize(move |task: &Task| Score::of_medium(amount(task, rule)))
        .named(rule.name())
}

fn soft(rule: Rule) -> impl IncrementalConstraint<Plan, Score> {
    ConstraintFactory::<Plan, Score>::new()
        .for_each(Plan::tasks())
        .filter(move |task: &Task| amount(task, rule) != 0)
        .penalize(move |task: &Task| Score::of_soft(amount(task, rule)))
        .named(rule.name())
}

/// A task that may end a resource's schedule: the last planned task of a
/// resource, or the last frozen one.
fn ends_resource(task: &Task) -> bool {
    task.slot.as_ref().is_some_and(Slot::placed)
        && (task.frozen_last || task.frozen_on.is_none() && task.next.is_none())
}

fn longest(spans: &CollectedVec<i64>) -> i64 {
    spans.iter().copied().max().unwrap_or(0).max(0)
}

pub fn constraints() -> impl ConstraintSet<Plan, Score> {
    (
        (
            hard(Rule::EarliestStart),
            hard(Rule::Deadline),
            hard(Rule::DependencyUnassigned),
            hard(Rule::DependencyTiming),
            hard(Rule::DependencySplit),
            hard(Rule::DependencyInterrupted),
            hard(Rule::TagMissing),
            hard(Rule::ResourceNotAllowed),
            hard(Rule::ForbiddenHandOver),
        ),
        medium(Rule::Unassigned),
        ConstraintFactory::<Plan, Score>::new()
            .for_each(Plan::tasks())
            .filter(ends_resource)
            .group_by(
                |_: &Task| 0usize,
                collect_vec(|task: &Task| task.slot.as_ref().map_or(0, |slot| slot.span)),
            )
            .penalize(|_: &usize, spans: &CollectedVec<i64>| Score::of_soft(longest(spans)))
            .named(Rule::Makespan.name()),
        (
            soft(Rule::IdealEnd),
            soft(Rule::Lateness),
            soft(Rule::ChangeoverTime),
            soft(Rule::ChangeoverCount),
            soft(Rule::IdleTime),
        ),
        ConstraintFactory::<Plan, Score>::new()
            .for_each(Plan::tasks())
            .filter(|task: &Task| amount(task, Rule::PreferredHandOver) != 0)
            .reward(|task: &Task| Score::of_soft(amount(task, Rule::PreferredHandOver)))
            .named(Rule::PreferredHandOver.name()),
    )
}
