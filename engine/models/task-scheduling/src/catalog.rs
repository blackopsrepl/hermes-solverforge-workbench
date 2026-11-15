//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::ConstraintInfo;

use crate::constraints::{Rule, RULES};

const TIME_WINDOWS: &str = "Time windows";
const DEPENDENCIES: &str = "Dependencies";
const FIT: &str = "Resource fit";
const HAND_OVERS: &str = "Resource hand-overs";
const THROUGHPUT: &str = "Throughput";
const DUE_DATES: &str = "Due dates";
const CHANGEOVERS: &str = "Changeovers";

/// The group and one-sentence description of a rule.
fn describe(rule: Rule) -> (&'static str, &'static str) {
    match rule {
        Rule::EarliestStart => (
            TIME_WINDOWS,
            "A task (pinned at a fixed start) starts before its earliest start; costs the minutes it is early.",
        ),
        Rule::Deadline => (
            TIME_WINDOWS,
            "A task ends after its deadline; costs the minutes it overruns.",
        ),
        Rule::DependencyUnassigned => (
            DEPENDENCIES,
            "A task is planned while a task it depends on is not: left unassigned or impossible to time; costs the broken-link penalty.",
        ),
        Rule::DependencyTiming => (
            DEPENDENCIES,
            "A task starts outside the offsets its dependency allows after the preceding task's end or start; costs the minutes outside.",
        ),
        Rule::DependencySplit => (
            DEPENDENCIES,
            "Two dependent tasks that must share a resource run on different ones; costs the broken-link penalty.",
        ),
        Rule::DependencyInterrupted => (
            DEPENDENCIES,
            "Other tasks run between two dependent tasks that must follow each other directly; costs the broken-link penalty per task in between.",
        ),
        Rule::TagMissing => (
            FIT,
            "A task runs on a resource that lacks a required tag, or holds it only part of the time; costs the minutes not covered.",
        ),
        Rule::ResourceNotAllowed => (
            FIT,
            "A task runs on a resource outside its required resources; costs the task's priority.",
        ),
        Rule::ForbiddenHandOver => (
            HAND_OVERS,
            "A task runs on a resource its preceding task may not hand over to from its own; costs the broken-link penalty.",
        ),
        Rule::Unassigned => (
            THROUGHPUT,
            "A task is left unassigned, or cannot be timed because it waits on a task queued after it on its resource or follows one that does; costs its priority, so the most important tasks are planned first.",
        ),
        Rule::Makespan => (
            THROUGHPUT,
            "The longest time any resource takes from its start to the end of its last task, in minutes.",
        ),
        Rule::IdealEnd => (
            DUE_DATES,
            "A task ends before or after its ideal end; costs the minutes off times its priority.",
        ),
        Rule::Lateness => (
            DUE_DATES,
            "A task ends after its due date; costs the minutes late.",
        ),
        Rule::ChangeoverTime => (
            CHANGEOVERS,
            "A machine spends setup time switching task types between consecutive tasks; costs the setup minutes.",
        ),
        Rule::ChangeoverCount => (
            CHANGEOVERS,
            "A machine switches task type, or sets up between consecutive tasks; costs one per changeover.",
        ),
        Rule::IdleTime => (
            THROUGHPUT,
            "A machine waits before a task without working, changing over or being unavailable; costs the idle minutes (off unless weighted).",
        ),
        Rule::PreferredHandOver => (
            HAND_OVERS,
            "A task runs on a resource its preceding task's resource prefers to hand over to; earns the hand-over's reward.",
        ),
    }
}

/// Every constraint with its group, level and a one-sentence description.
pub fn catalog() -> Vec<ConstraintInfo> {
    RULES
        .iter()
        .map(|&rule| {
            let (group, description) = describe(rule);
            ConstraintInfo {
                name: rule.name(),
                group,
                level: rule.level(),
                description,
            }
        })
        .collect()
}
