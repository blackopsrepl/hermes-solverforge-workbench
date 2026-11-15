//! `fill`: the best shifts for jobs the plan leaves unplanned, ranked by the
//! model's own score, or one of them applied.
//!
//! A job's place is a driver shift: its stops leave wherever they are (the
//! bench, or nowhere yet) and go into that shift one by one, each at the
//! position that scores best after the stops before it.

use std::cell::RefCell;

use solverforge::PlanningSolution;
use solverforge_workbench_host::fill::{FillAnswer, FillRequest, Ranking};
use solverforge_workbench_host::{EngineError, EntityName};

use crate::constraints::{define_constraints, Constraints};
use crate::domain::Plan;
use crate::model::PickupDeliveryRouting;

/// The shifts' entity descriptor.
const SHIFTS: usize = 0;

type Director = solverforge_workbench_host::fill::Director<PickupDeliveryRouting, Constraints>;
type Score = <Plan as PlanningSolution>::Score;

/// Where a visit sits: its shift and position.
fn locate(plan: &Plan, v: usize) -> Option<(usize, usize)> {
    plan.shifts.iter().enumerate().find_map(|(s, shift)| {
        shift
            .visits
            .iter()
            .position(|&x| x == v)
            .map(|position| (s, position))
    })
}

fn take(director: &mut Director, s: usize, position: usize) -> usize {
    let mut taken = 0;
    director.do_change(SHIFTS, s, |plan: &mut Plan| {
        taken = plan.shifts[s].visits.remove(position);
    });
    taken
}

fn put(director: &mut Director, s: usize, position: usize, v: usize) -> Score {
    director.do_change(SHIFTS, s, |plan: &mut Plan| {
        plan.shifts[s].visits.insert(position, v);
    })
}

/// What one tried placement moved, to undo it.
#[derive(Default)]
struct Moved {
    /// Where the job's stops came from, in the order they were taken.
    from: Vec<(usize, usize, usize)>,
    /// Where they went, in the order they were put.
    to: Vec<(usize, usize)>,
}

/// Takes `stops` out of their lists, then puts each into shift `s` at its
/// best position after the stops before it.
fn place(director: &mut Director, stops: &[usize], s: usize, moved: &mut Moved) -> Score {
    let mut found: Vec<(usize, usize, usize)> = stops
        .iter()
        .filter_map(|&v| locate(director.working_solution(), v).map(|(sh, p)| (sh, p, v)))
        .collect();
    // Later positions first, so earlier ones stay valid.
    found.sort_by(|a, b| (a.0, b.1).cmp(&(b.0, a.1)));
    for &(sh, p, v) in &found {
        take(director, sh, p);
        moved.from.push((sh, p, v));
    }
    let mut after = 0;
    let mut score = director.calculate_score();
    for &v in stops {
        let len = director.working_solution().shifts[s].visits.len();
        let mut best: Option<(usize, Score)> = None;
        for position in after..=len {
            let tried = put(director, s, position, v);
            take(director, s, position);
            if best.is_none_or(|(_, b)| tried > b) {
                best = Some((position, tried));
            }
        }
        let (position, _) = best.expect("a list always has a last position");
        score = put(director, s, position, v);
        moved.to.push((s, position));
        after = position + 1;
    }
    score
}

fn unplace(director: &mut Director, moved: &mut Moved) {
    for &(s, position) in moved.to.iter().rev() {
        take(director, s, position);
    }
    for &(s, position, v) in moved.from.iter().rev() {
        put(director, s, position, v);
    }
    *moved = Moved::default();
}

pub fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {
    let facts = plan.facts.clone();
    if facts.shifts.is_empty() {
        return Err(EngineError::new("unprepared", "the plan was not prepared"));
    }
    let bench = facts.bench();
    let by_id = |id: &str| facts.jobs.iter().position(|j| j.id == id);
    let unknown = |id: &str| EngineError::new("unknown_item", format!("no job {id:?} to fill"));
    let open = |j: usize| {
        facts.jobs[j]
            .stops
            .iter()
            .any(|&v| locate(&plan, v).is_none_or(|(s, _)| s == bench))
    };
    let items: Vec<usize> = match (&request.apply, request.items.is_empty()) {
        (Some(choice), _) => vec![by_id(&choice.item).ok_or_else(|| unknown(&choice.item))?],
        (None, true) => (0..facts.jobs.len()).filter(|&j| open(j)).collect(),
        (None, false) => request
            .items
            .iter()
            .map(|id| by_id(id).ok_or_else(|| unknown(id)))
            .collect::<Result<_, _>>()?,
    };
    for &j in &items {
        if facts.jobs[j]
            .stops
            .iter()
            .any(|&v| facts.visits[v].pin.is_some())
        {
            return Err(EngineError::new(
                "pinned_item",
                format!(
                    "job {:?} has pinned stops and cannot move",
                    facts.jobs[j].id
                ),
            ));
        }
    }
    let shifts: Vec<(usize, EntityName)> = (0..bench)
        .filter(|&s| !plan.shifts[s].pinned)
        .map(|s| {
            (
                s,
                EntityName {
                    id: Some(facts.shifts[s].id.clone()),
                    label: facts.shifts[s].label.clone(),
                },
            )
        })
        .collect();
    let mut ranking =
        Ranking::<PickupDeliveryRouting, Constraints>::new(plan, define_constraints());
    if let Some(choice) = &request.apply {
        let Some(&(s, _)) = shifts
            .iter()
            .find(|(_, name)| name.id.as_deref() == Some(choice.value.as_str()))
        else {
            return Err(EngineError::new(
                "unknown_value",
                format!("{:?} is not a shift for {:?}", choice.value, choice.item),
            ));
        };
        let stops = facts.jobs[items[0]].stops.clone();
        place(ranking.director(), &stops, s, &mut Moved::default());
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut answers = Vec::with_capacity(items.len());
    for j in items {
        let job = &facts.jobs[j];
        let item = EntityName {
            id: Some(job.id.clone()),
            label: job.label.clone(),
        };
        let (targets, names): (Vec<usize>, Vec<EntityName>) = shifts.iter().cloned().unzip();
        let moved = RefCell::new(Moved::default());
        answers.push(ranking.rank(
            "Job",
            item,
            names,
            request.top.max(1),
            &mut |director: &mut Director, n| {
                place(director, &job.stops, targets[n], &mut moved.borrow_mut())
            },
            &mut |director: &mut Director, _| unplace(director, &mut moved.borrow_mut()),
        ));
    }
    Ok(FillAnswer::Options(answers))
}
