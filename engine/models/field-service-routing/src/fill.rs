//! `fill`: the best shifts for visits the plan leaves unplanned, ranked by
//! the model's own score, or one of them applied.
//!
//! A visit's place is a shift: it leaves wherever it is (the bench, or
//! nowhere yet) and goes into that shift at the position that scores best.
//! Only shifts the visit may use are ranked: not a vehicle it rules out,
//! nor one outside whose coverage area it lies.

use std::cell::RefCell;

use solverforge::PlanningSolution;
use solverforge_workbench_host::fill::{FillAnswer, FillRequest, Ranking};
use solverforge_workbench_host::{EngineError, EntityName};

use crate::constraints::{define_constraints, Constraints};
use crate::domain::Plan;
use crate::model::FieldServiceRouting;

/// The shifts' entity descriptor.
const SHIFTS: usize = 0;

type Director = solverforge_workbench_host::fill::Director<FieldServiceRouting, Constraints>;
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
    from: Option<(usize, usize)>,
    to: Option<(usize, usize)>,
    visit: usize,
}

/// Takes visit `v` out of its list, then puts it into shift `s` at its best
/// position.
fn place(director: &mut Director, v: usize, s: usize, moved: &mut Moved) -> Score {
    moved.visit = v;
    if let Some((sh, p)) = locate(director.working_solution(), v) {
        take(director, sh, p);
        moved.from = Some((sh, p));
    }
    let len = director.working_solution().shifts[s].visits.len();
    let mut best: Option<(usize, Score)> = None;
    for position in 0..=len {
        let tried = put(director, s, position, v);
        take(director, s, position);
        if best.is_none_or(|(_, b)| tried > b) {
            best = Some((position, tried));
        }
    }
    let (position, _) = best.expect("a list always has a last position");
    moved.to = Some((s, position));
    put(director, s, position, v)
}

fn unplace(director: &mut Director, moved: &mut Moved) {
    if let Some((s, position)) = moved.to.take() {
        take(director, s, position);
    }
    if let Some((s, position)) = moved.from.take() {
        put(director, s, position, moved.visit);
    }
}

pub fn fill(plan: Plan, request: &FillRequest) -> Result<FillAnswer<Plan>, EngineError> {
    let facts = plan.facts.clone();
    if facts.shifts.is_empty() {
        return Err(EngineError::new("unprepared", "the plan was not prepared"));
    }
    let bench = facts.bench();
    let visits = facts.first_break;
    let by_id = |id: &str| facts.visits[..visits].iter().position(|v| v.id == id);
    let unknown = |id: &str| EngineError::new("unknown_item", format!("no visit {id:?} to fill"));
    let open = |v: usize| locate(&plan, v).is_none_or(|(s, _)| s == bench);
    let items: Vec<usize> = match (&request.apply, request.items.is_empty()) {
        (Some(choice), _) => vec![by_id(&choice.item).ok_or_else(|| unknown(&choice.item))?],
        (None, true) => (0..visits).filter(|&v| open(v)).collect(),
        (None, false) => request
            .items
            .iter()
            .map(|id| by_id(id).ok_or_else(|| unknown(id)))
            .collect::<Result<_, _>>()?,
    };
    for &v in &items {
        if facts.visits[v].pin.is_some() {
            return Err(EngineError::new(
                "pinned_item",
                format!("visit {:?} is pinned and cannot move", facts.visits[v].id),
            ));
        }
    }
    let pinned: Vec<bool> = plan.shifts.iter().map(|s| s.pinned).collect();
    let usable = |v: usize, s: usize| {
        let fit = facts.fit(v, s);
        !pinned[s] && !fit.not_required && !fit.prohibited && !fit.outside_required
    };
    let name = |s: usize| EntityName {
        id: Some(facts.shifts[s].id.clone()),
        label: facts.shifts[s].label.clone(),
    };
    let mut ranking = Ranking::<FieldServiceRouting, Constraints>::new(plan, define_constraints());
    if let Some(choice) = &request.apply {
        let v = items[0];
        let Some(s) = (0..bench).find(|&s| facts.shifts[s].id == choice.value) else {
            return Err(EngineError::new(
                "unknown_value",
                format!("{:?} is not a shift for {:?}", choice.value, choice.item),
            ));
        };
        place(ranking.director(), v, s, &mut Moved::default());
        return Ok(FillAnswer::Applied(ranking.into_plan()));
    }
    let mut answers = Vec::with_capacity(items.len());
    for v in items {
        let fact = &facts.visits[v];
        let item = EntityName {
            id: Some(fact.id.clone()),
            label: fact.label.clone(),
        };
        let targets: Vec<usize> = (0..bench).filter(|&s| usable(v, s)).collect();
        let names = targets.iter().map(|&s| name(s)).collect();
        let moved = RefCell::new(Moved::default());
        answers.push(ranking.rank(
            "Visit",
            item,
            names,
            request.top.max(1),
            &mut |director: &mut Director, n| {
                place(director, v, targets[n], &mut moved.borrow_mut())
            },
            &mut |director: &mut Director, _| unplace(director, &mut moved.borrow_mut()),
        ));
    }
    Ok(FillAnswer::Options(answers))
}
