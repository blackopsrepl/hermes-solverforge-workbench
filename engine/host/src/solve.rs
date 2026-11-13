//! `solve`: run one retained job and stream it as JSON Lines.
//!
//! stdin stays open after the request line. A `{"control":"cancel"}` line, or
//! stdin closing, cancels the solve; the best plan found so far is still
//! reported, analysed, as the final `done` event.

use std::io::{BufRead, Write};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::json;
use solverforge::config::SolverConfigOverride;
use solverforge::{SolverEvent, SolverEventMetadata, SolverTelemetry};

use crate::analysis::analyze;
use crate::model::{Kpi, Model, ScoreOf};
use crate::output::Output;
use crate::plan::{apply_weights, parse};
use crate::protocol::{EngineError, Request};

/// Shortest gap between two `progress` events.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);
/// Shortest gap between two `best` events that carry a full plan.
const BEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Deserialize)]
struct Control {
    control: String,
}

pub fn solve<M: Model, W: Write>(
    request: Request,
    controls: impl BufRead + Send + 'static,
    out: &mut Output<W>,
) -> Result<(), EngineError> {
    let (mut plan, issues) = parse::<M>(request.plan)?;
    apply_weights::<M>(&mut plan, &request.weights)?;
    let manager = M::manager();
    let started = match &request.termination {
        Some(termination) => manager.solve_with_override(
            plan,
            SolverConfigOverride::with_termination(termination.to_config()),
        ),
        None => manager.solve(plan),
    };
    let (job, mut receiver) =
        started.map_err(|error| EngineError::new("solve_rejected", error.to_string()))?;
    out.event("started", json!({ "model": M::id(), "issues": issues }));
    thread::spawn(move || watch_controls::<M>(controls, job));

    let mut last_progress: Option<Instant> = None;
    let mut last_best: Option<Instant> = None;
    while let Some(event) = receiver.blocking_recv() {
        match event {
            SolverEvent::Progress { metadata } => {
                if due(&mut last_progress, PROGRESS_INTERVAL) {
                    out.event("progress", progress::<M>(&metadata));
                }
            }
            SolverEvent::BestSolution { metadata, solution } => {
                if due(&mut last_best, BEST_INTERVAL) {
                    let mut fields = progress::<M>(&metadata);
                    fields["plan"] = serde_json::to_value(&solution).unwrap_or_default();
                    out.event("best", fields);
                }
            }
            SolverEvent::PauseRequested { .. }
            | SolverEvent::Paused { .. }
            | SolverEvent::Resumed { .. } => {}
            SolverEvent::Completed { metadata, solution } => {
                finish::<M, W>(
                    out,
                    "completed",
                    &metadata.telemetry,
                    solution,
                    &request.max_matches,
                );
                let _ = manager.delete(job);
                return Ok(());
            }
            SolverEvent::Cancelled { metadata } => {
                let snapshot = manager
                    .get_snapshot(job, None)
                    .map_err(|error| EngineError::new("snapshot_unavailable", error.to_string()))?;
                finish::<M, W>(
                    out,
                    "cancelled",
                    &metadata.telemetry,
                    snapshot.solution,
                    &request.max_matches,
                );
                let _ = manager.delete(job);
                return Ok(());
            }
            SolverEvent::Failed { error, .. } => {
                let _ = manager.delete(job);
                return Err(EngineError::new("solve_failed", error));
            }
        }
    }
    Err(EngineError::new(
        "solve_failed",
        "the solver stopped without a final event",
    ))
}

fn watch_controls<M: Model>(controls: impl BufRead, job: usize) {
    for line in controls.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(Control { control }) = serde_json::from_str::<Control>(&line) {
            if control == "cancel" {
                break;
            }
        }
    }
    // A cancel request, or the caller closing stdin, ends the solve. A job
    // that already finished rejects this, which is fine.
    let _ = M::manager().cancel(job);
}

fn due(last: &mut Option<Instant>, interval: Duration) -> bool {
    let now = Instant::now();
    if last.is_some_and(|at| now.duration_since(at) < interval) {
        return false;
    }
    *last = Some(now);
    true
}

fn progress<M: Model>(metadata: &SolverEventMetadata<ScoreOf<M>>) -> serde_json::Value {
    json!({
        "best_score": metadata.best_score.map(|score| score.to_string()),
        "current_score": metadata.current_score.map(|score| score.to_string()),
        "telemetry": telemetry(&metadata.telemetry),
    })
}

fn telemetry(telemetry: &SolverTelemetry) -> serde_json::Value {
    json!({
        "elapsed_ms": telemetry.elapsed.as_millis() as u64,
        "steps": telemetry.step_count,
        "moves_evaluated": telemetry.moves_evaluated,
        "moves_accepted": telemetry.moves_accepted,
    })
}

fn finish<M: Model, W: Write>(
    out: &mut Output<W>,
    status: &str,
    stats: &SolverTelemetry,
    solution: M::Plan,
    max_matches: &usize,
) {
    let kpis: Vec<Kpi> = M::kpis(&solution);
    let plan = serde_json::to_value(&solution).unwrap_or_default();
    let analysis = analyze::<M>(solution, *max_matches);
    out.event(
        "done",
        json!({
            "status": status,
            "score": analysis.score,
            "feasible": analysis.feasible,
            "telemetry": telemetry(stats),
            "plan": plan,
            "analysis": analysis.constraints,
            "kpis": kpis,
        }),
    );
}
