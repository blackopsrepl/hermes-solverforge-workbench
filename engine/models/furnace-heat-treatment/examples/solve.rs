//! Solves a demo through the engine and prints its progress and result:
//! `cargo run --release --example solve -- LARGE_WEEK 30`.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge::ConstraintSet;
use solverforge_workbench_furnace_heat_treatment::FurnaceHeatTreatment;
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

/// Keeps stdin open while the solve runs, so the engine does not cancel it.
struct Open(Vec<u8>, usize);

impl io::Read for Open {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.1 < self.0.len() {
            let n = buf.len().min(self.0.len() - self.1);
            buf[..n].copy_from_slice(&self.0[self.1..self.1 + n]);
            self.1 += n;
            return Ok(n);
        }
        std::thread::sleep(std::time::Duration::from_secs(3600));
        Ok(0)
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let demo = args.next().unwrap_or_else(|| "SMALL_SHOP".into());
    let seconds: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(30);
    let prepared = FurnaceHeatTreatment::demo(&demo).expect("demo");
    let initial = FurnaceHeatTreatment::constraints().evaluate_all(&prepared);
    println!(
        "{demo}: {} orders, initial score {initial}",
        prepared.work_orders.len()
    );
    let plan = FurnaceHeatTreatment::present(&prepared);
    let request = json!({"command": "solve", "model": "furnace-heat-treatment", "plan": plan,
                         "termination": {"seconds": seconds}, "max_matches": 3});
    let sink = Sink::default();
    let code = run_io(
        "example",
        &[entry::<FurnaceHeatTreatment>()],
        false,
        Open(format!("{request}\n").into_bytes(), 0),
        Box::new(sink.clone()),
    );
    let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        match event["event"].as_str() {
            Some("progress") | Some("best") => {}
            Some("done") | Some("completed") | Some("cancelled") => {
                let mut summary = event.clone();
                if let Some(obj) = summary.as_object_mut() {
                    obj.remove("plan");
                    obj.remove("analysis");
                }
                println!("{summary}");
                if let Some(analysis) = event["analysis"].as_array() {
                    for c in analysis {
                        if c["score"].as_str().is_some_and(|s| s.contains('-')) {
                            println!("  {:<45} {}", c["name"].as_str().unwrap_or(""), c["score"]);
                        }
                    }
                }
            }
            _ => println!("{line}"),
        }
    }
    std::process::exit(code);
}
