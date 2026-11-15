//! `fill`: the best places for tasks the plan leaves open, ranked by the
//! model's own score, or one of them applied.
//!
//! A task's place is a position in a resource's sequence, so a candidate is
//! `<resource id>@<position>`: the task moves out of where it is (nowhere
//! or the unscheduled queue for an open task) and in at that position.

use solverforge_workbench_host::fill::{name_of, FillAnswer, FillRequest, Ranking};
use solverforge_workbench_host::{EngineError, EntityName};

use crate::constraints::{define_constraints, Constraints};
use crate::domain::{Lane, Plan};
use crate::model::TaskScheduling;
use crate::names::label;

/// The resources' entity descriptor.
const RESOURCES: usize = 0;

type Director = solverforge_workbench_host::fill::Director<TaskScheduling, Constraints>;

/// One place a task can go: a resource and a position in its sequence.
#[derive(Clone, Copy)]
struct Place {
    resource: usize,
    position: usize,
}

/// Where task `t` sits: its owner and its index in the owner's sequence.
fn locate(plan: &Plan, t: usize) -> Option<Place> {
    plan.resources.iter().enumerate().find_map(|(resource, r)| {
        r.sequence
            .iter()
            .position(|&x| x == t)
            .map(|position| Place { resource, position })
    })
}

/// Whether task `t` is frozen.
fn frozen(plan: &Plan, t: usize) -> bool {
    plan.tasks[t].frozen_on.is_some()
}

fn take(director: &mut Director, place: Place) {
    director.do_change(RESOURCES, place.resource, |plan: &mut Plan| {
        plan.resources[place.resource]
            .sequence
            .remove(place.position);
    });
}

fn put(
    director: &mut Director,
    place: Place,
    t: usize,
) -> <Plan as solverforge::PlanningSolution>::Score {
    director.do_change(RESOURCES, place.resource, |plan: &mut Plan| {
        plan.resources[place.resource]
            .sequence
            .insert(place.position, t);
    })
}

/// Every place for task `t` on a resource it may run on, with its name.
fn places(plan: &Plan, t: usize) -> Vec<(Place, EntityName)> {
    let allowed = plan
        .facts
        .tasks
        .get(t)
        .and_then(|task| task.allowed.as_ref());
    let mut out = Vec::new();
    for (resource, r) in plan.resources.iter().enumerate() {
        if !r.is_resource()
            || allowed.is_some_and(|allowed| !allowed.get(resource).copied().unwrap_or(false))
        {
            continue;
        }
        let others: Vec<usize> = r.sequence.iter().copied().filter(|&x| x != t).collect();
        let name = label(plan, "Resource", resource);
        for position in 0..=others.len() {
            let before = match position {
                0 => r.frozen.last().copied(),
                p => Some(others[p - 1]),
            };
            let where_ = match before {
                Some(task) => format!("{name}, after {}", label(plan, "Task", task)),
                None => format!("{name}, first"),
            };
            out.push((
                Place { resource, position },
                EntityName {
                    id: Some(format!("{}@{position}", r.id)),
                    label: where_,
                },
            ));
        }
    }
    out
}

pub fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {
    if plan.resources.iter().all(|r| r.lane != Lane::Queue) {
        return Err(EngineError::new("unprepared", "the plan was not prepared"));
    }
    let queue = plan.facts.queue;
    let by_id = |id: &str| plan.tasks.iter().position(|task| task.id == id);
    let open = |t: &usize| match locate(&plan, *t) {
        Some(place) => place.resource == queue,
        None => !frozen(&plan, *t),
    };
    let mut items: Vec<usize> = if request.items.is_empty() {
        (0..plan.tasks.len()).filter(open).collect()
    } else {
        request
            .items
            .iter()
            .map(|id| {
                by_id(id).ok_or_else(|| {
                    EngineError::new("unknown_item", format!("no task {id:?} to fill"))
                })
            })
            .collect::<Result<_, _>>()?
    };
    if let Some(choice) = &request.apply {
        let t = by_id(&choice.item).ok_or_else(|| {
            EngineError::new("unknown_item", format!("no task {:?} to fill", choice.item))
        })?;
        items = vec![t];
    }
    let mut work = Vec::with_capacity(items.len());
    for &t in &items {
        if frozen(&plan, t) {
            return Err(EngineError::new(
                "pinned_item",
                format!("task {:?} is pinned and cannot move", plan.tasks[t].id),
            ));
        }
        let from = locate(&plan, t);
        work.push((t, from, places(&plan, t)));
    }
    let mut ranking = Ranking::<TaskScheduling, Constraints>::new(plan, define_constraints());
    if let Some(choice) = &request.apply {
        let (t, from, candidates) = &work[0];
        let Some((to, _)) = candidates
            .iter()
            .find(|(_, name)| name.id.as_deref() == Some(choice.value.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_value",
                format!("{:?} is not a place for {:?}", choice.value, choice.item),
            ));
        };
        if let Some(from) = from {
            take(ranking.director(), *from);
        }
        put(ranking.director(), *to, *t);
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut answers = Vec::with_capacity(work.len());
    for (t, from, candidates) in work {
        let item = name_of::<TaskScheduling>(ranking.plan(), "Task", t);
        let (targets, names): (Vec<Place>, Vec<EntityName>) = candidates.into_iter().unzip();
        answers.push(ranking.rank(
            "Task",
            item,
            names,
            request.top.max(1),
            &mut |director: &mut Director, n| {
                if let Some(from) = from {
                    take(director, from);
                }
                put(director, targets[n], t)
            },
            &mut |director: &mut Director, n| {
                take(director, targets[n]);
                if let Some(from) = from {
                    put(director, from, t);
                }
            },
        ));
    }
    Ok(FillAnswer::Options(answers))
}
