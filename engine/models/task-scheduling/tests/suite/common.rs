//! Helpers shared by the model's integration tests.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge::{ConstraintSet, HardMediumSoftScore, PlanningSolution, ScoreDirector};
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

/// The plan's score from scratch, its slots recomputed first.
pub fn total(plan: &Plan) -> Score {
    let mut fresh = plan.clone();
    fresh.update_all_shadows();
    define_constraints().evaluate_all(&fresh)
}

/// Every task's slot, recomputed from scratch.
pub fn fresh_slots(
    plan: &Plan,
) -> Vec<Option<solverforge_workbench_task_scheduling::schedule::Slot>> {
    let mut fresh = plan.clone();
    fresh.update_all_shadows();
    fresh.tasks.iter().map(|task| task.slot).collect()
}

/// Places every open task, one at a time in construction order, onto a
/// resource or the queue the way construction does, checking each step
/// against slots and a score computed from scratch.
pub fn assert_construction_matches_full(plan: Plan, seed: u64) {
    let owners = plan.resources.iter().filter(|r| !r.pinned).count();
    let mut open: Vec<usize> = (0..plan.tasks.len())
        .filter(|&t| plan.tasks[t].lane.is_none())
        .collect();
    open.sort_by_key(|&t| plan.facts.construction_key(t));
    let mut director = ScoreDirector::new(plan, define_constraints());
    director.calculate_score();
    let mut state = seed;
    let mut next = move |n: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % n.max(1)
    };
    for (step, t) in open.into_iter().enumerate() {
        let to = next(owners);
        let len = director.working_solution().resources[to].sequence.len();
        let at = next(len + 1);
        let incremental = director.do_change(0, to, |plan: &mut Plan| {
            plan.resources[to].sequence.insert(at, t);
        });
        let slots: Vec<_> = director
            .working_solution()
            .tasks
            .iter()
            .map(|t| t.slot)
            .collect();
        assert!(
            slots == fresh_slots(director.working_solution()),
            "step {step}: a slot is stale"
        );
        let full = total(director.working_solution());
        assert_eq!(
            incremental, full,
            "step {step}: incremental {incremental}, full {full}"
        );
    }
}

/// Random list moves between resources and the queue, scored
/// incrementally, each checked against slots and a score computed from
/// scratch. The frozen lane, last, never moves.
pub fn assert_incremental_matches_full(plan: Plan, steps: usize, seed: u64) {
    let mut plan = plan;
    let owners = plan.resources.iter().filter(|r| !r.pinned).count();
    // Open tasks start in no list: deal them round-robin over the
    // resources and the queue first.
    let open: Vec<usize> = (0..plan.tasks.len())
        .filter(|&t| plan.tasks[t].lane.is_none())
        .collect();
    assert!(!open.is_empty() || plan.resources.iter().any(|r| !r.sequence.is_empty()));
    for (n, t) in open.into_iter().enumerate() {
        plan.resources[n % owners].sequence.push(t);
    }
    plan.update_all_shadows();
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
        // Both owners change: the director refreshes each one's list.
        director.before_variable_changed(0, from);
        if to != from {
            director.before_variable_changed(0, to);
        }
        {
            let plan = director.working_solution_mut();
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
        }
        director.after_variable_changed(0, from);
        if to != from {
            director.after_variable_changed(0, to);
        }
        let incremental = director.calculate_score();
        let slots: Vec<_> = director
            .working_solution()
            .tasks
            .iter()
            .map(|t| t.slot)
            .collect();
        let fresh = fresh_slots(director.working_solution());
        if slots != fresh {
            let plan = director.working_solution();
            for (t, (a, b)) in slots.iter().zip(&fresh).enumerate() {
                if a != b {
                    let task = &plan.tasks[t];
                    eprintln!(
                        "task {t} lane {:?} prev {:?} deps {:?}\n  have {:?}\n  want {:?}",
                        task.lane,
                        task.previous,
                        plan.facts.tasks[t]
                            .deps
                            .iter()
                            .map(|d| d.task)
                            .collect::<Vec<_>>(),
                        a,
                        b
                    );
                }
            }
            panic!("step {step}: a slot is stale (swap {swap}, from {from}, to {to})");
        }
        let full = total(director.working_solution());
        assert_eq!(
            incremental, full,
            "step {step}: incremental {incremental}, full {full}"
        );
    }
}

/// Applies JSON Patch operations whose paths may address records by id
/// (`[id=X]`), the way the workbench applies a model's changes.
pub fn apply(plan: &mut Value, operations: &[Value]) {
    for op in operations {
        let path = op["path"].as_str().unwrap();
        let mut parts: Vec<String> = path[1..]
            .split('/')
            .map(|p| p.replace("~1", "/").replace("~0", "~"))
            .collect();
        let last = parts.pop().unwrap();
        let mut at = &mut *plan;
        for part in &parts {
            at = step(at, part);
        }
        match op["op"].as_str().unwrap() {
            "remove" => match at {
                Value::Array(items) => {
                    let i = index(items, &last);
                    items.remove(i);
                }
                Value::Object(fields) => {
                    fields
                        .remove(&last)
                        .unwrap_or_else(|| panic!("no field {last}"));
                }
                other => panic!("cannot remove {last} from {other}"),
            },
            "add" if last == "-" => at.as_array_mut().unwrap().push(op["value"].clone()),
            "add" | "replace" if at.is_object() => {
                at.as_object_mut()
                    .unwrap()
                    .insert(last, op["value"].clone());
            }
            "add" | "replace" => *step(at, &last) = op["value"].clone(),
            other => panic!("unsupported op {other}"),
        }
    }
}

fn index(items: &[Value], part: &str) -> usize {
    match part.strip_prefix("[id=").and_then(|p| p.strip_suffix(']')) {
        Some(id) => items
            .iter()
            .position(|item| item["id"] == id)
            .unwrap_or_else(|| panic!("no {id}")),
        None => part.parse().unwrap(),
    }
}

fn step<'a>(at: &'a mut Value, part: &str) -> &'a mut Value {
    if at.is_array() {
        let i = index(at.as_array().unwrap(), part);
        &mut at[i]
    } else {
        at.get_mut(part)
            .unwrap_or_else(|| panic!("no field {part}"))
    }
}
