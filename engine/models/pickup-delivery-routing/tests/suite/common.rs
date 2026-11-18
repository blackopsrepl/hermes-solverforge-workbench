//! Helpers shared by the model's integration tests.

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge::{ConstraintSet, HardMediumSoftScore, PlanningSolution, ScoreDirector};
use solverforge_workbench_host::roads::{provide_complete, same, Place, Roads};
use solverforge_workbench_host::{entry, run_io, Model, Severity};
use solverforge_workbench_pickup_delivery_routing::constraints::define_constraints;
use solverforge_workbench_pickup_delivery_routing::{PickupDeliveryRouting, Plan};

pub type Score = HardMediumSoftScore;

/// Monday 1 February 2027 (day 0) or a later day, at `hh:mm`.
pub fn at(day: i64, hour: i64, minute: i64) -> String {
    format!("2027-02-{:02}T{hour:02}:{minute:02}", day + 1)
}

/// The depot every test shift starts and ends at.
pub const DEPOT: [f64; 2] = [45.4846, 9.2367];

/// A dataset of drivers and jobs, plus any top-level fields.
pub fn dataset(drivers: Vec<Value>, jobs: Vec<Value>, extra: Value) -> Value {
    let mut out = json!({"drivers": drivers, "jobs": jobs});
    merge(&mut out, extra);
    out
}

fn merge(out: &mut Value, extra: Value) {
    for (key, value) in extra.as_object().unwrap() {
        out[key] = value.clone();
    }
}

/// A driver with one shift.
pub fn driver(id: &str, shift: Value) -> Value {
    json!({"id": id, "shifts": [shift]})
}

/// A shift starting at the depot on Monday 08:00, driving `itinerary`
/// (stop ids; `brk:` names a break).
pub fn shift(id: &str, itinerary: &[&str], extra: Value) -> Value {
    let items: Vec<Value> = itinerary
        .iter()
        .map(|item| match item.strip_prefix("brk:") {
            Some(b) => json!({"id": b, "kind": "BREAK"}),
            None => json!({"id": item, "kind": "STOP"}),
        })
        .collect();
    let mut out = json!({
        "id": id, "startLocation": DEPOT, "minStartTime": at(0, 8, 0), "itinerary": items,
    });
    merge(&mut out, extra);
    out
}

/// A job of stops, in service order.
pub fn job(id: &str, stops: Vec<Value>, extra: Value) -> Value {
    let mut out = json!({"id": id, "stops": stops});
    merge(&mut out, extra);
    out
}

/// A stop at the depot unless `extra` moves it.
pub fn stop(id: &str, minutes: i64, extra: Value) -> Value {
    let mut out = json!({"id": id, "location": DEPOT, "duration": minutes});
    merge(&mut out, extra);
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

/// The request line, then a caller that keeps stdin open: closing it would
/// cancel a solve.
struct Held {
    line: io::Cursor<Vec<u8>>,
}

impl io::Read for Held {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.line.read(buf)? {
            0 => {
                std::thread::sleep(std::time::Duration::from_secs(3600));
                Ok(0)
            }
            n => Ok(n),
        }
    }
}

/// Sends one request through the engine; returns its exit code and last event.
pub fn engine(body: Value) -> (i32, Value) {
    request(body, false)
}

/// Solves a dataset for `seconds`; returns the exit code and the `done` event.
pub fn solve(plan: Value, seconds: u64) -> (i32, Value) {
    request(
        json!({"command": "solve", "plan": plan, "termination": {"seconds": seconds}}),
        true,
    )
}

/// Great-circle meters between two `[lat, lon]` points.
pub fn haversine(a: [f64; 2], b: [f64; 2]) -> f64 {
    let (la1, lo1, la2, lo2) = (
        a[0].to_radians(),
        a[1].to_radians(),
        b[0].to_radians(),
        b[1].to_radians(),
    );
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * 6_371_000.0 * h.sqrt().asin()
}

/// The roads the tests drive on: a road between every two places, 1.35 times
/// the straight line, driven at 32 km/h.
pub fn test_leg(a: [f64; 2], b: [f64; 2]) -> Option<(f64, f64)> {
    let length = haversine(a, b) * 1.35;
    Some((length / (32.0 / 3.6), length))
}

/// Provides the test roads for every place of a wire plan, so the engine reads
/// them instead of fetching a city.
pub fn give_roads(wire: &Value) {
    let Ok(plan) = serde_json::from_value::<Plan>(wire.clone()) else {
        return;
    };
    let points: Vec<[f64; 2]> = PickupDeliveryRouting::places(&plan)
        .iter()
        .map(|p| p.point)
        .collect();
    if !points.is_empty() {
        provide_complete(&points, test_leg);
    }
}

fn request(mut body: Value, hold: bool) -> (i32, Value) {
    body["model"] = json!("pickup-delivery-routing");
    if body.get("roads").is_none() {
        give_roads(&body["plan"]);
    }
    let sink = Sink::default();
    let line = io::Cursor::new(format!("{body}\n").into_bytes());
    let input: Box<dyn io::Read + Send> = if hold {
        Box::new(Held { line })
    } else {
        Box::new(line)
    };
    let code = run_io(
        "test",
        &[entry::<PickupDeliveryRouting>()],
        false,
        input,
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

/// A wire dataset as a prepared plan.
pub fn prepared(wire: Value) -> Plan {
    give_roads(&wire);
    let mut plan: Plan = serde_json::from_value(wire).unwrap();
    if plan.travel_matrix.is_none() {
        let mut distinct: Vec<Place> = Vec::new();
        for place in PickupDeliveryRouting::places(&plan) {
            if !distinct.iter().any(|p| same(&p.point, &place.point)) {
                distinct.push(place);
            }
        }
        let points: Vec<[f64; 2]> = distinct.iter().map(|p| p.point).collect();
        let (matrix, _) = Roads::load(None, &points)
            .unwrap()
            .matrix(&distinct)
            .unwrap();
        PickupDeliveryRouting::set_roads(&mut plan, matrix);
    }
    let issues = PickupDeliveryRouting::prepare(&mut plan);
    assert!(
        issues.iter().all(|i| i.severity != Severity::Error),
        "{issues:?}"
    );
    plan
}

/// The demo as a prepared plan, the way the engine reads it.
pub fn demo_plan() -> Plan {
    prepared(PickupDeliveryRouting::present(
        &PickupDeliveryRouting::demo("MILAN_TWO_DAYS").unwrap(),
    ))
}

/// The plan's score from scratch, shadows recomputed.
pub fn total(plan: &Plan) -> Score {
    let mut fresh = plan.clone();
    fresh.update_all_shadows();
    define_constraints().evaluate_all(&fresh)
}

/// Spreads every visit over the shifts and the bench at random, breaks
/// included.
pub fn scramble(plan: &mut Plan, seed: u64) {
    let owners = plan.shifts.len();
    let mut state = seed;
    for shift in &mut plan.shifts {
        shift.visits.clear();
    }
    for v in 0..plan.visits.len() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let s = ((state >> 33) as usize) % owners;
        plan.shifts[s].visits.push(v);
    }
}

/// Random list moves (change and swap, within and between shifts, and in
/// and out of the lists entirely), scored incrementally, each checked
/// against a full evaluation.
pub fn assert_incremental_matches_full(plan: Plan, steps: usize, seed: u64) {
    let owners = plan.shifts.len();
    let mut director = ScoreDirector::new(plan, define_constraints());
    director.calculate_score();
    let mut state = seed;
    let mut next = move |n: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % n.max(1)
    };
    let mut out: Vec<usize> = Vec::new();
    for step in 0..steps {
        let from = next(owners);
        let to = next(owners);
        let len = director.working_solution().shifts[from].visits.len();
        let kind = next(10);
        if kind == 0 && len > 0 {
            // Take a visit out of every list.
            let at = next(len);
            director.before_variable_changed(0, from);
            let v = director.working_solution_mut().shifts[from]
                .visits
                .remove(at);
            director.after_variable_changed(0, from);
            out.push(v);
        } else if kind == 1 && !out.is_empty() {
            // Put one back.
            let v = out.swap_remove(next(out.len()));
            let target_len = director.working_solution().shifts[to].visits.len();
            let at = next(target_len + 1);
            director.before_variable_changed(0, to);
            director.working_solution_mut().shifts[to]
                .visits
                .insert(at, v);
            director.after_variable_changed(0, to);
        } else if len > 0 {
            let at = next(len);
            let target_len = director.working_solution().shifts[to].visits.len();
            let swap = kind < 5 && from != to && target_len > 0;
            director.before_variable_changed(0, from);
            if from != to {
                director.before_variable_changed(0, to);
            }
            let plan = director.working_solution_mut();
            if swap {
                let other = (at + to) % target_len;
                let a = plan.shifts[from].visits[at];
                let b = plan.shifts[to].visits[other];
                plan.shifts[from].visits[at] = b;
                plan.shifts[to].visits[other] = a;
            } else {
                let v = plan.shifts[from].visits.remove(at);
                let list = &mut plan.shifts[to].visits;
                let position = (at * 7 + step) % (list.len() + 1);
                list.insert(position, v);
            }
            director.after_variable_changed(0, from);
            if from != to {
                director.after_variable_changed(0, to);
            }
        } else {
            continue;
        }
        let incremental = director.calculate_score();
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
