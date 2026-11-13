//! Drives every engine command end to end on a small real model.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use solverforge_workbench_host::{entry, run_io};
use solverforge_workbench_testkit::toy;

/// Collects what the engine writes.
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

impl Sink {
    fn events(&self) -> Vec<Value> {
        let bytes = self.0.lock().unwrap().clone();
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value.get("event").is_some())
            .collect()
    }
}

/// Runs one request to completion; input ends right after it.
fn request(body: Value) -> (i32, Vec<Value>) {
    let sink = Sink::default();
    let input = io::Cursor::new(format!("{body}\n").into_bytes());
    let code = run_io(
        "test",
        &[entry::<toy::Toy>()],
        false,
        input,
        Box::new(sink.clone()),
    );
    (code, sink.events())
}

fn last(events: &[Value]) -> &Value {
    events.last().expect("the engine wrote no event")
}

fn plan_json(roster: toy::Roster) -> Value {
    serde_json::to_value(roster).unwrap()
}

#[test]
fn version_lists_the_models() {
    let sink = Sink::default();
    let code = run_io(
        "9.9.9",
        &[entry::<toy::Toy>()],
        true,
        io::empty(),
        Box::new(sink.clone()),
    );
    assert_eq!(code, 0);
    let events = sink.events();
    assert_eq!(events[0]["version"], "9.9.9");
    assert_eq!(events[0]["models"][0]["id"], "toy");
}

#[test]
fn an_unknown_model_is_refused_with_the_known_ones() {
    let (code, events) = request(json!({"command": "describe", "model": "nope"}));
    assert_eq!(code, 1);
    assert_eq!(last(&events)["error"]["code"], "unknown_model");
    assert_eq!(last(&events)["error"]["detail"]["models"][0], "toy");
}

#[test]
fn describe_merges_the_catalog_with_the_constraint_set() {
    let (code, events) = request(json!({"command": "describe", "model": "toy"}));
    assert_eq!(code, 0);
    let described = last(&events);
    assert_eq!(described["weights"], true);
    assert_eq!(described["demos"][0], "WEEK");
    let constraints = described["constraints"].as_array().unwrap();
    assert_eq!(constraints.len(), 3);
    assert_eq!(constraints[0]["name"], "Unassigned shift");
    assert_eq!(constraints[0]["group"], "Coverage");
    assert_eq!(constraints[2]["level"], "soft");
}

#[test]
fn demo_returns_the_default_dataset() {
    let (code, events) = request(json!({"command": "demo", "model": "toy"}));
    assert_eq!(code, 0);
    assert_eq!(last(&events)["id"], "WEEK");
    assert_eq!(last(&events)["plan"]["shifts"].as_array().unwrap().len(), 6);
}

#[test]
fn evaluate_reports_every_match_up_to_the_cap() {
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": plan_json(toy::week()), "max_matches": 2
    }));
    assert_eq!(code, 0);
    let evaluated = last(&events);
    assert_eq!(evaluated["score"], "-6hard/0soft");
    assert_eq!(evaluated["feasible"], false);
    let unassigned = &evaluated["analysis"][0];
    assert_eq!(unassigned["match_count"], 6);
    assert_eq!(unassigned["matches"].as_array().unwrap().len(), 2);
    assert_eq!(unassigned["matches"][0]["entities"][0]["type"], "Shift");
    assert_eq!(evaluated["kpis"][0]["value"], 0.0);
}

#[test]
fn a_zero_weight_switches_a_constraint_off() {
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": plan_json(toy::week()),
        "weights": {"Unassigned shift": 0}
    }));
    assert_eq!(code, 0);
    assert_eq!(last(&events)["score"], "0hard/0soft");
    assert_eq!(last(&events)["analysis"][0]["weight"], 0);
}

#[test]
fn a_misspelt_weight_is_refused_with_suggestions() {
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": plan_json(toy::week()),
        "weights": {"Unasigned shift": 3}
    }));
    assert_eq!(code, 1);
    let error = &last(&events)["error"];
    assert_eq!(error["code"], "unknown_constraint");
    assert_eq!(error["detail"]["suggestions"][0], "Unassigned shift");
}

#[test]
fn a_plan_of_the_wrong_shape_is_refused() {
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": {"nurses": "none"}
    }));
    assert_eq!(code, 1);
    assert_eq!(last(&events)["error"]["code"], "invalid_plan");
}

#[test]
fn a_dataset_error_refuses_the_request_with_every_issue() {
    let mut roster = toy::week();
    roster.shifts[0].nurse_idx = Some(7);
    roster.nurses.truncate(1);
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": plan_json(roster)
    }));
    assert_eq!(code, 1);
    let error = &last(&events)["error"];
    assert_eq!(error["code"], "invalid_dataset");
    let issues = error["detail"]["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 2);
    assert_eq!(issues[0]["code"], "unknown_nurse");
    assert_eq!(issues[1]["severity"], "warning");
}

#[test]
fn warnings_travel_with_the_result() {
    let mut roster = toy::week();
    roster.nurses.truncate(1);
    let (code, events) = request(json!({
        "command": "evaluate", "model": "toy", "plan": plan_json(roster)
    }));
    assert_eq!(code, 0);
    assert_eq!(last(&events)["issues"][0]["code"], "few_nurses");
}

#[test]
fn solve_finishes_feasible_and_keeps_pinned_assignments() {
    let mut roster = toy::week();
    roster.shifts[0].nurse_idx = Some(0);
    roster.shifts[0].pinned = true;
    let (code, events) = request_held(
        json!({
            "command": "solve", "model": "toy", "plan": plan_json(roster),
            "termination": {"seconds": 1}
        }),
        Duration::from_secs(20),
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(events[0]["event"], "started");
    let done = last(&events);
    assert_eq!(done["event"], "done");
    assert_eq!(done["status"], "completed");
    assert_eq!(done["score"], "0hard/-1soft");
    assert_eq!(done["plan"]["shifts"][0]["nurse_idx"], 0);
    assert_eq!(done["kpis"][0]["value"], 6.0);
}

#[test]
fn a_cancel_line_ends_the_solve_with_the_best_plan() {
    let (code, events) = request_held(
        json!({
            "command": "solve", "model": "toy", "plan": plan_json(toy::week()),
            "termination": {"seconds": 120}
        }),
        Duration::from_secs(30),
        Some(Duration::from_millis(1500)),
    );
    assert_eq!(code, 0);
    let done = last(&events);
    assert_eq!(done["status"], "cancelled");
    assert!(done["telemetry"]["elapsed_ms"].as_u64().unwrap() < 30_000);
    assert_eq!(done["feasible"], true);
}

/// Runs a solve whose input stays open, optionally sending a cancel line
/// after `cancel_after`, and closes input once the engine returns.
fn request_held(body: Value, limit: Duration, cancel_after: Option<Duration>) -> (i32, Vec<Value>) {
    let (reader, mut writer) = io::pipe().unwrap();
    writeln!(writer, "{body}").unwrap();
    let sink = Sink::default();
    let engine_sink = sink.clone();
    let engine = thread::spawn(move || {
        run_io(
            "test",
            &[entry::<toy::Toy>()],
            false,
            reader,
            Box::new(engine_sink),
        )
    });
    if let Some(delay) = cancel_after {
        thread::sleep(delay);
        writeln!(writer, "{}", json!({"control": "cancel"})).unwrap();
    }
    let started = std::time::Instant::now();
    while !engine.is_finished() {
        assert!(
            started.elapsed() < limit,
            "the engine did not finish in {limit:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
    drop(writer);
    (engine.join().unwrap(), sink.events())
}
