//! Facts derived from the dataset: times in minutes, durations per resource,
//! tags per resource, the changeover and hand-over tables.

use solverforge_workbench_host::Issue;

use super::ids::Ids;
use crate::derived::{Calendar, Dep, Facts, ResourceFacts, TaskFacts, Transition};
use crate::domain::Plan;
use crate::time::parse_date_time;
use crate::wire::{Anchor, ResourceKind};

/// Every id index of a dataset.
pub struct Index<'p> {
    pub tasks: Ids<'p>,
    pub resources: Ids<'p>,
    pub tags: Ids<'p>,
    pub task_types: Ids<'p>,
    pub machine_types: Ids<'p>,
}

impl<'p> Index<'p> {
    pub fn new(plan: &'p Plan, issues: &mut Vec<Issue>) -> Self {
        Self {
            tasks: Ids::new("task", plan.tasks.iter().map(|t| t.id.as_str()), issues),
            resources: Ids::new(
                "resource",
                plan.resources.iter().map(|r| r.id.as_str()),
                issues,
            ),
            tags: Ids::new("tag", plan.tags.iter().map(|t| t.id.as_str()), issues),
            task_types: Ids::new(
                "task type",
                plan.task_types.iter().map(|t| t.id.as_str()),
                issues,
            ),
            machine_types: Ids::new(
                "machine type",
                plan.machine_types.iter().map(|t| t.id.as_str()),
                issues,
            ),
        }
    }
}

/// Parses an optional local date-time, reporting a malformed one.
pub fn time(text: Option<&str>, context: &str, issues: &mut Vec<Issue>) -> Option<i64> {
    let text = text?;
    let parsed = parse_date_time(text);
    if parsed.is_none() {
        issues.push(Issue::error(
            "invalid_time",
            format!("{context} {text:?} is not a local date-time YYYY-MM-DDTHH:MM"),
        ));
    }
    parsed
}

fn resource_facts(plan: &Plan, index: &Index, issues: &mut Vec<Issue>) -> Vec<ResourceFacts> {
    let tag_count = plan.tags.len();
    plan.resources
        .iter()
        .map(|resource| {
            let who = format!("resource {:?}", resource.id);
            let start = time(
                Some(&resource.start),
                &format!("the start of {who}"),
                issues,
            )
            .unwrap_or(0);
            let machine = resource.kind == ResourceKind::Machine;
            if !machine && (resource.machine_type.is_some() || resource.initial_task_type.is_some())
            {
                issues.push(Issue::warning(
                    "employee_machine_type",
                    format!(
                        "{who} is an employee: its machine type and initial task type are ignored"
                    ),
                ));
            }
            let machine_type = resource
                .machine_type
                .as_deref()
                .and_then(|id| index.machine_types.find(id, &who, issues));
            let initial_type = resource
                .initial_task_type
                .as_deref()
                .and_then(|id| index.task_types.find(id, &who, issues));
            let mut spans = Vec::new();
            for span in &resource.unavailable {
                let from = time(
                    Some(&span.start),
                    &format!("an unavailable span of {who}"),
                    issues,
                );
                let to = time(
                    Some(&span.end),
                    &format!("an unavailable span of {who}"),
                    issues,
                );
                if let (Some(from), Some(to)) = (from, to) {
                    if to <= from {
                        issues.push(Issue::error(
                            "empty_span",
                            format!("{who} has an unavailable span that ends before it starts"),
                        ));
                    } else {
                        spans.push((from, to));
                    }
                }
            }
            let calendar = Calendar::new(spans.clone());
            let total: i64 = spans.iter().map(|(a, b)| b - a).sum();
            let merged: i64 = calendar.spans().iter().map(|(a, b)| b - a).sum();
            if merged != total {
                issues.push(Issue::warning(
                    "overlapping_spans",
                    format!("{who} has overlapping unavailable spans; they are merged"),
                ));
            }
            let mut tags = vec![None; tag_count];
            for tag in &resource.tags {
                let Some(t) = index.tags.find(&tag.tag, &who, issues) else {
                    continue;
                };
                if tags[t].is_some() {
                    issues.push(Issue::error(
                        "duplicate_tag",
                        format!("{who} lists tag {:?} twice; give it one span", tag.tag),
                    ));
                }
                let from = time(
                    tag.from.as_deref(),
                    &format!("tag {:?} of {who}", tag.tag),
                    issues,
                )
                .unwrap_or(i64::MIN);
                let until = time(
                    tag.until.as_deref(),
                    &format!("tag {:?} of {who}", tag.tag),
                    issues,
                )
                .unwrap_or(i64::MAX);
                if until <= from {
                    issues.push(Issue::error(
                        "empty_span",
                        format!("tag {:?} of {who} ends before it starts", tag.tag),
                    ));
                }
                tags[t] = Some((from, until));
            }
            ResourceFacts {
                start,
                machine,
                machine_type: machine_type.filter(|_| machine),
                initial_type: initial_type.filter(|_| machine),
                calendar,
                tags,
            }
        })
        .collect()
}

fn task_facts(plan: &Plan, index: &Index, issues: &mut Vec<Issue>) -> Vec<TaskFacts> {
    let resources = plan.resources.len();
    let parameters = &plan.parameters;
    plan.tasks
        .iter()
        .map(|task| {
            let who = format!("task {:?}", task.id);
            let mut overrides = vec![None; resources];
            for on in &task.duration_on {
                if on.duration < 0 {
                    issues.push(Issue::error(
                        "negative_duration",
                        format!("{who} has a negative duration on {:?}", on.resource),
                    ));
                }
                if let Some(r) = index.resources.find(&on.resource, &who, issues) {
                    overrides[r] = Some(on.duration.max(0));
                }
            }
            if task.duration.is_some_and(|d| d < 0) {
                issues.push(Issue::error(
                    "negative_duration",
                    format!("{who} has a negative duration"),
                ));
            }
            let mut allowed: Option<Vec<bool>> = (!task.required_resources.is_empty()).then(|| {
                let mut allowed = vec![false; resources];
                for id in &task.required_resources {
                    if let Some(r) = index.resources.find(id, &who, issues) {
                        allowed[r] = true;
                    }
                }
                allowed
            });
            let duration: Vec<i64> = match task.duration {
                Some(default) => overrides.iter().map(|o| o.unwrap_or(default.max(0))).collect(),
                None if task.duration_on.is_empty() => {
                    issues.push(Issue::error(
                        "missing_duration",
                        format!("{who} has neither a duration nor a duration on any resource"),
                    ));
                    vec![0; resources]
                }
                None => {
                    // Only the listed resources know how long it takes.
                    let longest = overrides.iter().flatten().copied().max().unwrap_or(0);
                    let listed: Vec<bool> = overrides.iter().map(Option::is_some).collect();
                    allowed = Some(match allowed {
                        Some(required) => required.iter().zip(&listed).map(|(a, b)| *a && *b).collect(),
                        None => listed,
                    });
                    overrides.iter().map(|o| o.unwrap_or(longest)).collect()
                }
            };
            let required_tags = task
                .required_tags
                .iter()
                .filter_map(|id| index.tags.find(id, &who, issues))
                .collect();
            let priority = task.priority.unwrap_or(parameters.default_priority);
            if priority < 1 {
                issues.push(Issue::error(
                    "invalid_priority",
                    format!("{who} has priority {priority}; priorities start at 1"),
                ));
            }
            let task_type = task
                .task_type
                .as_deref()
                .and_then(|id| index.task_types.find(id, &who, issues));
            let deps = task
                .dependencies
                .iter()
                .filter_map(|dep| {
                    let pred = index.tasks.find(&dep.task, &who, issues)?;
                    let min_offset = dep.min_offset.unwrap_or(0);
                    if min_offset < 0 || dep.max_offset.is_some_and(|m| m < 0) {
                        issues.push(Issue::error(
                            "negative_offset",
                            format!("{who} has a negative offset on its dependency {:?}", dep.task),
                        ));
                    }
                    if dep.max_offset.is_some_and(|max| max < min_offset) {
                        issues.push(Issue::error(
                            "empty_offset_range",
                            format!("{who} allows no offset after {:?}: its maximum is below its minimum", dep.task),
                        ));
                    }
                    if dep.no_intermediate_tasks && !dep.same_resource {
                        issues.push(Issue::warning(
                            "no_intermediate_without_same_resource",
                            format!("{who} forbids tasks between it and {:?} without requiring the same resource; it applies only when both share one", dep.task),
                        ));
                    }
                    if dep.same_resource && dep.anchor != Anchor::End {
                        issues.push(Issue::warning(
                            "same_resource_from_start",
                            format!("{who} needs the resource of {:?} but counts from its start; they cannot overlap on one resource", dep.task),
                        ));
                    }
                    if pred == task.index {
                        issues.push(Issue::error(
                            "self_dependency",
                            format!("{who} depends on itself"),
                        ));
                        return None;
                    }
                    Some(Dep {
                        task: pred,
                        from_end: dep.anchor == Anchor::End,
                        min_offset: min_offset.max(0),
                        max_offset: dep.max_offset.map(|m| m.max(0)),
                        same_resource: dep.same_resource,
                        no_intermediate: dep.no_intermediate_tasks,
                    })
                })
                .collect();
            TaskFacts {
                duration,
                allowed,
                required_tags,
                min_start: time(task.min_start.as_deref(), &format!("the earliest start of {who}"), issues),
                max_end: time(task.max_end.as_deref(), &format!("the deadline of {who}"), issues),
                ideal_end: time(task.ideal_end.as_deref(), &format!("the ideal end of {who}"), issues),
                due: time(task.due_date.as_deref(), &format!("the due date of {who}"), issues),
                priority: priority.max(1),
                task_type,
                interruptible: !task.no_interruptions.unwrap_or(!parameters.interruptible),
                deps,
                depth: 0,
            }
        })
        .collect()
}

/// Dependency depth of every task, or `None` when dependencies form a cycle.
fn depths(tasks: &[TaskFacts]) -> Option<Vec<i64>> {
    let n = tasks.len();
    let mut depth = vec![0i64; n];
    let mut pending: Vec<usize> = tasks.iter().map(|t| t.deps.len()).collect();
    let mut dependents = vec![Vec::new(); n];
    for (t, facts) in tasks.iter().enumerate() {
        for dep in &facts.deps {
            dependents[dep.task].push(t);
        }
    }
    let mut ready: Vec<usize> = (0..n).filter(|&t| pending[t] == 0).collect();
    let mut seen = 0;
    while let Some(t) = ready.pop() {
        seen += 1;
        for &d in &dependents[t] {
            depth[d] = depth[d].max(depth[t] + 1);
            pending[d] -= 1;
            if pending[d] == 0 {
                ready.push(d);
            }
        }
    }
    (seen == n).then_some(depth)
}

/// Derives every fact of `plan`; the unscheduled queue is not yet in it.
pub fn derive(plan: &Plan, index: &Index, issues: &mut Vec<Issue>) -> Facts {
    let resources = resource_facts(plan, index, issues);
    let mut tasks = task_facts(plan, index, issues);
    match depths(&tasks) {
        Some(depth) => {
            for (facts, depth) in tasks.iter_mut().zip(depth) {
                facts.depth = depth;
            }
        }
        None => issues.push(Issue::error(
            "dependency_cycle",
            "task dependencies form a cycle, so no order can satisfy them",
        )),
    }
    let mut dependents = vec![Vec::new(); tasks.len()];
    for (t, facts) in tasks.iter().enumerate() {
        for (i, dep) in facts.deps.iter().enumerate() {
            dependents[dep.task].push((t, i));
        }
    }
    let types = index.task_types.len().max(plan.task_types.len());
    let machine_types = plan.machine_types.len();
    let mut changeover = vec![None; machine_types * types * types];
    for (n, entry) in plan.changeovers.iter().enumerate() {
        let who = format!("changeover {n}");
        let machine_type = index.machine_types.find(&entry.machine_type, &who, issues);
        let from = index.task_types.find(&entry.from, &who, issues);
        let to = index.task_types.find(&entry.to, &who, issues);
        if entry.duration < 0 {
            issues.push(Issue::error(
                "negative_duration",
                format!(
                    "{who} ({} → {}) has a negative duration",
                    entry.from, entry.to
                ),
            ));
        }
        if let (Some(m), Some(a), Some(b)) = (machine_type, from, to) {
            let slot = &mut changeover[(m * types + a) * types + b];
            if slot.is_some() {
                issues.push(Issue::warning(
                    "duplicate_changeover",
                    format!(
                        "{who} repeats {} → {} on {}; the last one counts",
                        entry.from, entry.to, entry.machine_type
                    ),
                ));
            }
            *slot = Some(entry.duration.max(0));
        }
    }
    let r = plan.resources.len();
    let mut transitions = vec![Transition::default(); r * r];
    for entry in &plan.resource_transitions {
        let who = format!("the hand-over {} → {}", entry.from, entry.to);
        let from = index.resources.find(&entry.from, &who, issues);
        let to = index.resources.find(&entry.to, &who, issues);
        if entry.forbidden && entry.reward.is_some() {
            issues.push(Issue::error(
                "forbidden_with_reward",
                format!("{who} is both forbidden and rewarded"),
            ));
        }
        if entry.reward.is_some_and(|reward| reward <= 0) {
            issues.push(Issue::error(
                "invalid_reward",
                format!("{who} has a reward below 1; forbid it instead"),
            ));
        }
        if !entry.forbidden && entry.reward.is_none() {
            issues.push(Issue::warning(
                "idle_transition",
                format!("{who} is neither forbidden nor rewarded"),
            ));
        }
        if let (Some(a), Some(b)) = (from, to) {
            transitions[a * r + b] = Transition {
                reward: entry.reward.unwrap_or(0).max(0),
                forbidden: entry.forbidden,
            };
        }
    }
    if plan.parameters.broken_link_penalty < 1 {
        issues.push(Issue::error(
            "invalid_parameter",
            "brokenLinkPenalty must be at least 1",
        ));
    }
    Facts {
        tasks,
        resources,
        queue: r,
        dependents,
        task_types: types,
        changeover,
        transitions,
        broken_link: plan.parameters.broken_link_penalty.max(1),
    }
}
