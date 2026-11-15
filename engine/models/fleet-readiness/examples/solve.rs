//! Solves a demo end to end through the engine contract and reports the
//! starting and final score with the solver's speed.
//!
//! `cargo run --release --example solve -- [DEMO] [SECONDS]`

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use solverforge_workbench_fleet_readiness::FleetReadiness;
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

/// The request line, then input that never ends: closing stdin would cancel
/// the solve before its time is up.
struct Held(io::Cursor<Vec<u8>>);

impl Read for Held {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.0.read(buf)?;
        if n == 0 {
            loop {
                std::thread::park();
            }
        }
        Ok(n)
    }
}

fn run(request: Value) -> Vec<Value> {
    let sink = Sink::default();
    let code = run_io(
        "example",
        &[entry::<FleetReadiness>()],
        false,
        Held(io::Cursor::new(format!("{request}\n").into_bytes())),
        Box::new(sink.clone()),
    );
    let text = String::from_utf8(sink.0.lock().unwrap().clone()).unwrap();
    assert_eq!(code, 0, "{text}");
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let demo = args.next().unwrap_or_else(|| "SMALL_FORTNIGHT".into());
    let seconds: u64 = args.next().map_or(30, |s| s.parse().expect("seconds"));
    let plan = FleetReadiness::present(&FleetReadiness::demo(&demo).expect("demo"));
    let evaluated = run(json!({"command": "evaluate", "model": "fleet-readiness", "plan": plan}));
    let start = evaluated.last().unwrap()["score"].clone();
    let events = run(json!({
        "command": "solve", "model": "fleet-readiness", "plan": plan,
        "termination": {"seconds": seconds},
    }));
    let done = events
        .iter()
        .find(|e| e["event"] == "done")
        .expect("the solve finished");
    let telemetry = &done["telemetry"];
    let elapsed = telemetry["elapsed_ms"].as_f64().unwrap_or(1.0).max(1.0) / 1000.0;
    let steps = telemetry["steps"].as_f64().unwrap_or(0.0);
    let moves = telemetry["moves_evaluated"].as_f64().unwrap_or(0.0);
    println!(
        "demo {demo}: {start} -> {} feasible={}",
        done["score"], done["feasible"]
    );
    println!(
        "elapsed {elapsed:.1}s, {:.0} steps/s, {:.0} moves/s",
        steps / elapsed,
        moves / elapsed
    );
    for kpi in done["kpis"].as_array().unwrap() {
        println!("  {}: {} {}", kpi["label"], kpi["value"], kpi["unit"]);
    }
    for c in done["analysis"].as_array().unwrap() {
        if c["score"]
            .as_str()
            .is_some_and(|s| s != "0hard/0medium/0soft")
        {
            println!("  {} = {}", c["name"], c["score"]);
            if c["level"] != "soft" {
                for found in c["matches"].as_array().unwrap().iter().take(3) {
                    let labels: Vec<&str> = found["entities"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|e| e["label"].as_str())
                        .collect();
                    println!("      {}: {}", found["score"], labels.join(" | "));
                }
            }
        }
    }
}
