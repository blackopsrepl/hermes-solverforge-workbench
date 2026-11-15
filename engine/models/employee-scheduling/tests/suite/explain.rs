//! Score explanations through the engine: every blamed record comes back by
//! its id and a label people read.

use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::EmployeeScheduling;
use solverforge_workbench_host::{entry, run_io, Model};

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

fn evaluate(plan: Value) -> Value {
    let sink = Sink::default();
    let body = json!({"command": "evaluate", "model": "employee-scheduling", "plan": plan, "max_matches": 1000});
    let code = run_io(
        "test",
        &[entry::<EmployeeScheduling>()],
        false,
        io::Cursor::new(format!("{body}\n").into_bytes()),
        Box::new(sink.clone()),
    );
    assert_eq!(code, 0);
    let bytes = sink.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .next_back()
        .expect("the engine answered")
}

#[test]
fn every_blamed_shift_and_employee_is_named_by_id_and_label() {
    // The small week with everyone's shifts on one person: overlaps, runs,
    // limits and open shifts all have matches to explain.
    let mut plan = EmployeeScheduling::present(&EmployeeScheduling::demo("SMALL_WEEK").unwrap());
    let first = plan["employees"][0]["id"].clone();
    for (n, shift) in plan["shifts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        shift["employee"] = if n % 4 == 3 {
            Value::Null
        } else {
            first.clone()
        };
    }
    let ids = |key: &str| -> HashSet<String> {
        plan[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["id"].as_str().unwrap().to_owned())
            .collect()
    };
    let (shifts, employees) = (ids("shifts"), ids("employees"));
    let answer = evaluate(plan.clone());
    let mut named = 0;
    for constraint in answer["analysis"].as_array().unwrap() {
        for found in constraint["matches"].as_array().unwrap() {
            for entity in found["entities"].as_array().unwrap() {
                let label = entity["label"].as_str().unwrap();
                assert!(!label.contains('{'), "{label}");
                let known = match entity["type"].as_str().unwrap() {
                    "Shift" => &shifts,
                    "Employee" => &employees,
                    _ => continue,
                };
                assert!(known.contains(entity["id"].as_str().unwrap()), "{entity}");
                named += 1;
            }
        }
    }
    assert!(named > 30, "only {named} shifts and employees were blamed");
}
