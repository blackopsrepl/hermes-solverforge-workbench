//! `describe`, `demo` and `evaluate`: the commands that answer at once.

use std::io::Write;

use serde_json::json;
use solverforge::{ConstraintSet, PlanningSolution};

use crate::analysis::analyze;
use crate::fill::FillAnswer;
use crate::model::{Kpi, Level, Model};
use crate::output::Output;
use crate::plan::{apply_weights, parse};
use crate::protocol::{EngineError, Request};

pub fn describe<M: Model, W: Write>(out: &mut Output<W>) {
    let catalog = M::catalog();
    let constraints = M::constraints();
    let entries: Vec<_> = constraints
        .constraint_metadata()
        .iter()
        .map(|meta| {
            let name = meta.constraint_ref.full_name();
            let info = catalog.iter().find(|info| info.name == name);
            let level = info.map(|info| info.level).unwrap_or(if meta.is_hard {
                Level::Hard
            } else {
                Level::Soft
            });
            json!({
                "name": name,
                "level": level,
                "group": info.map(|info| info.group),
                "description": info.map(|info| info.description),
            })
        })
        .collect();
    let has_weights = <M::Plan as PlanningSolution>::HAS_CONSTRAINT_WEIGHTS;
    out.event(
        "described",
        json!({
            "model": M::id(),
            "name": M::name(),
            "demos": M::demos(),
            "weights": has_weights,
            "parameters": M::parameters(),
            "constraints": entries,
        }),
    );
}

pub fn demo<M: Model, W: Write>(request: &Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let demos = M::demos();
    let id = request
        .demo
        .as_deref()
        .or(demos.first().copied())
        .ok_or_else(|| {
            EngineError::new("no_demo", format!("model {} has no demo data", M::id()))
        })?;
    let plan = M::demo(id).ok_or_else(|| {
        EngineError::new(
            "unknown_demo",
            format!("model {} has no demo {id:?}", M::id()),
        )
        .with_detail(json!({ "demos": demos }))
    })?;
    out.event("demo", json!({ "id": id, "plan": M::present(&plan) }));
    Ok(())
}

pub fn evaluate<M: Model, W: Write>(
    request: Request,
    out: &mut Output<W>,
) -> Result<(), EngineError> {
    let (mut plan, issues) = parse::<M>(request.plan, request.freeze_before.as_deref())?;
    apply_weights::<M>(&mut plan, &request.weights)?;
    let kpis: Vec<Kpi> = M::kpis(&plan);
    let analysis = analyze::<M>(plan, request.max_matches);
    out.event(
        "evaluated",
        json!({
            "score": analysis.score,
            "feasible": analysis.feasible,
            "analysis": analysis.constraints,
            "kpis": kpis,
            "issues": issues,
        }),
    );
    Ok(())
}

/// Most matches read when a view marks broken items.
const VIEW_MATCHES: usize = 10_000;

pub fn view<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let (mut plan, _) = parse::<M>(request.plan, request.freeze_before.as_deref())?;
    apply_weights::<M>(&mut plan, &request.weights)?;
    let analysis = analyze::<M>(plan.clone(), VIEW_MATCHES);
    let view = M::view(&plan, &analysis).ok_or_else(|| {
        EngineError::new("no_view", format!("model {} has no plan view", M::id()))
    })?;
    out.event("view", json!({ "view": view }));
    Ok(())
}

pub fn fill<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let (mut plan, issues) = parse::<M>(request.plan, request.freeze_before.as_deref())?;
    apply_weights::<M>(&mut plan, &request.weights)?;
    let fill = request.fill.unwrap_or_default();
    match M::fill(plan, &fill)? {
        FillAnswer::Options(items) => out.event("filled", json!({ "items": items, "issues": issues })),
        FillAnswer::Applied(plan) => out.event(
            "filled",
            json!({ "applied": fill.apply.map(|c| json!({"item": c.item, "value": c.value})), "plan": M::present(&plan) }),
        ),
    }
    Ok(())
}
