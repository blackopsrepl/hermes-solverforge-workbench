//! Helpers shared by the model's integration tests.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge::{ConstraintSet, HardMediumSoftScore, ScoreDirector};
use solverforge_workbench_host::{entry, run_io, Model};
use solverforge_workbench_task_scheduling::constraints::define_constraints;
use solverforge_workbench_task_scheduling::{Plan, TaskScheduling};

pub type Score = HardMediumSoftScore;

/// Monday 1 March 2027 at `hh:mm`.
pub fn at(hour: i64, minute: i64) -> String {
    format!("2027-03-01T{hour:02}:{minute:02}")
}

/// A dataset from parts; resources start at 08:00 unless they say otherwise.
pub fn dataset(parts: Value) -> Value {
    let mut base = json!({"tasks": [], "resources": []});
    for (key, value) in parts.as_object().unwrap() {
        base[key] = value.clone();
    }
    base
}

pub fn machine(id: &str, tasks: &[&str], extra: Value) -> Value {
    resource(id, "machine", tasks, extra)
}

pub fn employee(id: &str, tasks: &[&str], extra: Value) -> Value {
    resource(id, "employee", tasks, extra)
}

fn resource(id: &str, kind: &str, tasks: &[&str], extra: Value) -> Value {
    let mut out = json!({
        "id": id, "kind": kind, "start": at(8, 0),
        "tasks": tasks.iter().map(|t| json!({"task": t})).collect::<Vec<_>>(),
    });
    for (key, value) in extra.as_object().unwrap() {
        out[key] = value.clone();
    }
    out
}

pub fn task(id: &str, minutes: i64, extra: Value) -> Value {
    let mut out = json!({"id": id, "duration": minutes});
    for (key, value) in extra.as_object().unwrap() {
        out[key] = value.clone();
    }
    out
}

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Sends one request through the engine; returns its exit code and last event.
pub fn engine(mut body: Value) -> (i32, Value) {
    body["model"] = json!("task-scheduling");
    let sink = Sink::default();
    let code = run_io(
        "test",
        &[entry::<TaskScheduling>()],
        false,
        io::Cursor::new(format!("{body}\n").into_bytes()),
        Box::new(sink.clone()),
    );
    let bytes = sink.0.lock().unwrap().clone();
    let last = String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .next_back()
        .expect("the engine answered");
    (code, last)
}

/// Evaluates a dataset; panics when the engine refuses it.
pub fn evaluate(plan: Value) -> Value {
    let (code, answer) = engine(json!({"command": "evaluate", "plan": plan, "max_matches": 100}));
    assert_eq!(code, 0, "{answer}");
    answer
}

/// Every constraint's score as text, by name.
pub fn scores(plan: Value) -> HashMap<String, String> {
    evaluate(plan)["analysis"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["name"].as_str().unwrap().to_string(),
                c["score"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// One constraint's score as text.
pub fn score(plan: Value, name: &str) -> String {
    scores(plan)
        .remove(name)
        .unwrap_or_else(|| panic!("no constraint {name:?}"))
}

/// Codes of the issues the engine reports for a dataset, errors included.
pub fn issue_codes(plan: Value) -> Vec<String> {
    let (_, answer) = engine(json!({"command": "evaluate", "plan": plan}));
    let issues = answer["issues"]
        .as_array()
        .or_else(|| answer["error"]["detail"]["issues"].as_array())
        .cloned()
        .unwrap_or_default();
    issues
        .iter()
        .map(|i| i["code"].as_str().unwrap().to_string())
        .collect()
}

/// A demo as a prepared plan, the way the engine reads it.
pub fn demo_plan(id: &str) -> Plan {
    let wire = TaskScheduling::present(&TaskScheduling::demo(id).unwrap());
    let mut plan: Plan = serde_json::from_value(wire).unwrap();
    let issues = TaskScheduling::prepare(&mut plan);
    assert!(
        issues
            .iter()
            .all(|i| i.severity != solverforge_workbench_host::Severity::Error),
        "{issues:?}"
    );
    plan
}

/// The plan's score from scratch.
pub fn total(plan: &Plan) -> Score {
    define_constraints().evaluate_all(plan)
}

/// Random list moves between resources, scored incrementally, each checked
/// against a full evaluation.
pub fn assert_incremental_matches_full(plan: Plan, steps: usize, seed: u64) {
    let owners = plan.resources.len();
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
        let from = next(owners);
        let len = director.working_solution().resources[from].sequence.len();
        if len == 0 {
            continue;
        }
        let at = next(len);
        let to = next(owners);
        let target_len = director.working_solution().resources[to].sequence.len();
        let swap = next(2) == 0 && from != to && target_len > 0;
        let other = next(target_len.max(1));
        let into = next(target_len + usize::from(from != to));
        let incremental = director.do_change(0, from, |plan: &mut Plan| {
            if swap {
                let a = plan.resources[from].sequence[at];
                let b = plan.resources[to].sequence[other];
                plan.resources[from].sequence[at] = b;
                plan.resources[to].sequence[other] = a;
            } else {
                let task = plan.resources[from].sequence.remove(at);
                let list = &mut plan.resources[to].sequence;
                let position = into.min(list.len());
                list.insert(position, task);
            }
        });
        let full = total(director.working_solution());
        assert_eq!(
            incremental, full,
            "step {step}: incremental {incremental}, full {full}"
        );
    }
}
