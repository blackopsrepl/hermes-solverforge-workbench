//! Helpers shared by the model's integration tests.
#![allow(dead_code)]

use std::collections::HashMap;

use serde_json::{json, Value};
use solverforge::{ConstraintSet, HardMediumSoftScore, ScoreDirector};
use solverforge_workbench_employee_scheduling::constraints::define_constraints;
use solverforge_workbench_employee_scheduling::prepare::prepare;
use solverforge_workbench_employee_scheduling::Plan;
use solverforge_workbench_host::Severity;

pub type Score = HardMediumSoftScore;

/// A one-week schedule starting Monday 2027-03-01, with the given parts.
pub fn dataset(mut parts: Value) -> Value {
    let base = json!({
        "schedule": {"start": "2027-03-01T00:00", "end": "2027-03-08T00:00"},
        "employees": [],
        "shifts": [],
    });
    let mut merged = base;
    for (key, value) in parts.as_object_mut().unwrap().iter_mut() {
        merged[key] = value.take();
    }
    merged
}

/// Deserializes and prepares a dataset; panics on any error-level issue.
pub fn plan(dataset: Value) -> Plan {
    let mut plan: Plan = serde_json::from_value(dataset).expect("dataset matches the model");
    let issues = prepare(&mut plan);
    let errors: Vec<_> = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    plan
}

/// Issue codes `prepare` reports for a dataset.
pub fn issue_codes(dataset: Value) -> Vec<&'static str> {
    let mut plan: Plan = serde_json::from_value(dataset).expect("dataset matches the model");
    prepare(&mut plan)
        .into_iter()
        .map(|issue| issue.code)
        .collect()
}

/// Score of every constraint, by name.
pub fn scores(plan: &Plan) -> HashMap<String, Score> {
    let mut director = ScoreDirector::new(plan.clone(), define_constraints());
    director.calculate_score();
    director
        .constraint_results()
        .into_iter()
        .map(|result| (result.name.to_owned(), result.score))
        .collect()
}

/// Score of one constraint; panics when the model has no such constraint.
pub fn score(plan: &Plan, name: &str) -> Score {
    *scores(plan)
        .get(name)
        .unwrap_or_else(|| panic!("no constraint named {name:?}"))
}

/// Total score from a full evaluation.
pub fn total(plan: &Plan) -> Score {
    define_constraints().evaluate_all(plan)
}

pub fn hard(n: i64) -> Score {
    Score::of_hard(n)
}

pub fn medium(n: i64) -> Score {
    Score::of(0, n, 0)
}

pub fn soft(n: i64) -> Score {
    Score::of_soft(n)
}

/// A minimal shift record.
pub fn shift(id: &str, start: &str, end: &str, employee: Option<&str>) -> Value {
    json!({"id": id, "start": start, "end": end, "employee": employee})
}

/// A shift record with extra fields merged in.
pub fn shift_with(id: &str, start: &str, end: &str, employee: Option<&str>, extra: Value) -> Value {
    let mut record = shift(id, start, end, employee);
    for (key, value) in extra.as_object().unwrap() {
        record[key] = value.clone();
    }
    record
}

/// An employee record with extra fields merged in.
pub fn employee(id: &str, extra: Value) -> Value {
    let mut record = json!({ "id": id });
    for (key, value) in extra.as_object().unwrap() {
        record[key] = value.clone();
    }
    record
}

/// Day `d` (0 = Monday 2027-03-01) at `hh:mm`.
pub fn at(day: i64, hour: i64, minute: i64) -> String {
    let total = day * 1440 + hour * 60 + minute;
    let (d, rest) = (total.div_euclid(1440), total.rem_euclid(1440));
    let date = 1 + d;
    if (1..=31).contains(&date) {
        format!("2027-03-{date:02}T{:02}:{:02}", rest / 60, rest % 60)
    } else if date < 1 {
        format!("2027-02-{:02}T{:02}:{:02}", 28 + date, rest / 60, rest % 60)
    } else {
        format!("2027-04-{:02}T{:02}:{:02}", date - 31, rest / 60, rest % 60)
    }
}

/// Walks `steps` seeded pseudo-random assignment changes through an
/// incremental score director and checks every score against a full evaluation.
pub fn assert_incremental_matches_full(plan: Plan, steps: usize, seed: u64) {
    let employees = plan.employees.len();
    let shifts = plan.shifts.len();
    if employees == 0 || shifts == 0 {
        return;
    }
    let movable: Vec<usize> = (0..shifts).filter(|&i| !plan.shifts[i].pinned).collect();
    let mut director = ScoreDirector::new(plan, define_constraints());
    director.calculate_score();
    let mut state = seed;
    let mut next = move |n: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % n.max(1)
    };
    for step in 0..steps {
        let shift = movable[next(movable.len())];
        let value = match next(employees + 2) {
            n if n >= employees => None,
            n => Some(n),
        };
        let incremental = director.do_change(0, shift, |plan: &mut Plan| {
            plan.shifts[shift].employee_idx = value;
        });
        let full = total(director.working_solution());
        assert_eq!(
            incremental, full,
            "step {step}: shift {shift} -> {value:?} scored {incremental} incrementally, {full} in full"
        );
    }
}
