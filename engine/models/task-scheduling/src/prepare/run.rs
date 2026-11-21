//! `prepare`: check a dataset and derive what scoring and solving read.

use std::sync::Arc;

use solverforge::PlanningSolution;
use solverforge_workbench_host::{Issue, Severity};

use super::facts::{derive, time, Index};
use super::place::place;
use crate::constraints::Rule;
use crate::derived::Facts;
use crate::domain::{Lane, Plan, Resource};
use crate::materials::{MaterialUse, Stock};
use crate::schedule::{begin, frozen_end, run};
use solverforge_workbench_host::stock::{Part, PartNames};

/// Checks `plan`, derives its facts and places its tasks. On any error the
/// plan is left without facts and must not be scored.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    plan.resources.retain(Resource::is_resource);
    for (index, task) in plan.tasks.iter_mut().enumerate() {
        task.index = index;
        task.pinned_start = None;
        task.frozen_on = None;
        task.frozen_previous = None;
        task.frozen_rank = 0;
        task.frozen_last = false;
        task.lane = None;
        task.previous = None;
        task.next = None;
        task.slot = None;
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
    let materials = materials(plan, &mut issues);
    if issues.iter().any(|issue| issue.severity == Severity::Error) {
        return issues;
    }
    warn_unschedulable(plan, &facts, &mut issues);

    for (t, start) in placed.pinned_start.into_iter().enumerate() {
        plan.tasks[t].pinned_start = start;
    }
    for &t in &placed.queued {
        plan.tasks[t].pinned = false;
    }
    for (r, (frozen, sequence)) in placed.frozen.into_iter().zip(placed.sequence).enumerate() {
        let mut previous = None;
        for (rank, &t) in frozen.iter().enumerate() {
            let task = &mut plan.tasks[t];
            task.pinned = true;
            task.frozen_on = Some(r);
            task.frozen_previous = previous;
            task.frozen_rank = rank;
            task.frozen_last = rank + 1 == frozen.len();
            previous = Some(t);
        }
        plan.resources[r].frozen = frozen;
        plan.resources[r].sequence = sequence;
    }
    for (task, tf) in plan.tasks.iter_mut().zip(&facts.tasks) {
        task.open_cost = tf.priority;
        task.link_cost = facts.broken_link;
    }
    for (task, materials) in plan.tasks.iter_mut().zip(materials) {
        task.materials = materials;
    }
    let queue = plan.resources.len();
    // Open tasks start on no list at all, so construction places every one
    // of them where it scores best: on a resource or in the queue.
    plan.resources
        .push(Resource::lane(queue, Lane::Queue, "Unscheduled"));
    let mut frozen_lane = Resource::lane(queue + 1, Lane::Frozen, "Frozen");
    frozen_lane.sequence = plan.resources[..queue]
        .iter()
        .flat_map(|resource| resource.frozen.iter().copied())
        .collect();
    plan.resources.push(frozen_lane);
    plan.facts = Arc::new(facts);
    crate::freeze::order_frozen_lane(plan);
    start_frozen_tasks(plan);
    // Idle time is off unless a weight turns it on.
    if plan.weights.get(Rule::IdleTime.name()).is_none() {
        plan.weights.set(Rule::IdleTime.name(), 0);
    }
    plan.score = None;
    plan.update_all_shadows();
    issues
}

/// Per task, the parts it uses up and makes with the plan's stock; `None`
/// for tasks that neither use nor make parts.
fn materials(plan: &Plan, issues: &mut Vec<Issue>) -> Vec<Option<Arc<MaterialUse>>> {
    let mut names = PartNames::default();
    let mut read = |parts: &[Part], what: String, issues: &mut Vec<Issue>| {
        names.read(parts, &what).unwrap_or_else(|message| {
            issues.push(Issue::error("bad_parts", message));
            Vec::new()
        })
    };
    let uses: Vec<_> = plan
        .tasks
        .iter()
        .map(|task| {
            let who = format!("task {:?}", task.id);
            (
                read(&task.consumes, who.clone(), issues),
                read(&task.produces, who, issues),
            )
        })
        .collect();
    let mut arrivals: Vec<(usize, i64, i64)> = Vec::new();
    for arrival in &plan.stock {
        let what = format!("stock of {:?}", arrival.part);
        let part = Part {
            part: arrival.part.clone(),
            quantity: arrival.quantity,
        };
        let Some(&(index, quantity)) =
            read(std::slice::from_ref(&part), what.clone(), issues).first()
        else {
            continue;
        };
        let at = match &arrival.at {
            Some(text) => match time(Some(text), &format!("the arrival of {what}"), issues) {
                Some(at) => at,
                None => continue,
            },
            None => i64::MIN,
        };
        arrivals.push((index, at, quantity));
    }
    let mut stock = Stock {
        arrivals: vec![Vec::new(); names.names.len()],
    };
    for (part, at, quantity) in arrivals {
        stock.arrivals[part].push((at, quantity));
    }
    let stock = Arc::new(stock);
    uses.into_iter()
        .map(|(consumes, produces)| {
            (!consumes.is_empty() || !produces.is_empty()).then(|| {
                Arc::new(MaterialUse {
                    consumes,
                    produces,
                    stock: Arc::clone(&stock),
                })
            })
        })
        .collect()
}

/// A frozen task the dataset gives no start starts as early as its
/// resource allows after the frozen task before it.
fn start_frozen_tasks(plan: &mut Plan) {
    let facts = Arc::clone(&plan.facts);
    for (r, rf) in facts.resources.iter().enumerate() {
        for i in 0..plan.resources[r].frozen.len() {
            let t = plan.resources[r].frozen[i];
            if plan.tasks[t].pinned_start.is_some() {
                continue;
            }
            let tf = &facts.tasks[t];
            let (ready, previous_type) = match plan.tasks[t].frozen_previous {
                Some(p) => (frozen_end(plan, p), facts.tasks[p].task_type),
                None => (rf.start, rf.initial_type),
            };
            let (_, earliest) = begin(&facts, rf, tf, ready, previous_type);
            plan.tasks[t].pinned_start = Some(run(rf, tf, earliest, tf.duration[r]).0);
        }
    }
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
