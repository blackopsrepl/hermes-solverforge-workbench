//! `describe`, `demo` and `evaluate`: the commands that answer at once.

use std::io::Write;

use serde_json::json;
use solverforge::{ConstraintSet, PlanningSolution};

use crate::analysis::analyze;
use crate::fill::FillAnswer;
use crate::model::{Kpi, Level, Model};
use crate::output::Output;
use crate::plan::{apply_weights, measured, parse};
use crate::protocol::{EngineError, Request};
use solverforge_maps::{encode_polyline, Coord};

use crate::roads::{unique, Roads, RoadsConfig};
use crate::view::{Route, Routes, View};

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
            "summary": M::summary(),
            "demos": M::demos(),
            "demo_info": M::demo_info(),
            "size": M::size_fields(),
            "weights": has_weights,
            "parameters": M::parameters(),
            "view": M::view_kind(),
            "roads": M::travels(),
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
    let (mut plan, issues) = parse::<M>(
        request.plan,
        request.freeze_before.as_deref(),
        request.roads.as_ref(),
    )?;
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
    let roads = request.roads.clone();
    let (mut plan, _) = parse::<M>(
        request.plan,
        request.freeze_before.as_deref(),
        roads.as_ref(),
    )?;
    apply_weights::<M>(&mut plan, &request.weights)?;
    let analysis = analyze::<M>(plan.clone(), VIEW_MATCHES);
    let mut view = M::view(&plan, &analysis).ok_or_else(|| {
        EngineError::new("no_view", format!("model {} has no plan view", M::id()))
    })?;
    if let View::Routes(routes) = &mut view {
        let places: Vec<[f64; 2]> = M::places(&plan).into_iter().map(|p| p.point).collect();
        along_roads(routes, &places, roads.as_ref())?;
    }
    out.event("view", json!({ "view": view }));
    Ok(())
}

/// Draws every route along the roads, from its start through its stops to its
/// end, on the network of the plan's places: the one its travel was measured on.
fn along_roads(
    routes: &mut Routes,
    places: &[[f64; 2]],
    config: Option<&RoadsConfig>,
) -> Result<(), EngineError> {
    let hops = |route: &Route| -> Vec<[f64; 2]> {
        std::iter::once([route.start.lat, route.start.lon])
            .chain(route.stops.iter().map(|s| [s.lat, s.lon]))
            .chain(std::iter::once([route.end.lat, route.end.lon]))
            .collect()
    };
    if places.is_empty() || routes.vehicles.is_empty() {
        return Ok(());
    }
    let roads = Roads::load(config, &unique(places))?;
    for route in &mut routes.vehicles {
        let path: Vec<Coord> = roads
            .path(&hops(route))?
            .into_iter()
            .map(|[lat, lng]| Coord::new(lat, lng))
            .collect();
        route.path = encode_polyline(&path);
    }
    Ok(())
}

/// The plan with its travel measured along the roads, for the caller to keep,
/// and what the roads say about its places. Nothing else about the plan is
/// checked: a plan with other issues is measured all the same.
/// Without a plan, the named demo is measured: how release builds fetch the
/// demos' road networks that ship with the engine.
pub fn roads<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let plan = match (request.plan, request.demo.as_deref()) {
        (Some(plan), _) => Some(plan),
        (None, Some(id)) => Some(M::present(&M::demo(id).ok_or_else(|| {
            EngineError::new(
                "unknown_demo",
                format!("model {} has no demo {id:?}", M::id()),
            )
        })?)),
        (None, None) => None,
    };
    let (plan, issues) = measured::<M>(plan, request.roads.as_ref())?;
    out.event(
        "roads",
        json!({ "plan": M::present(&plan), "issues": issues }),
    );
    Ok(())
}

/// The plan as given, deserialized but not prepared: what replanning changes are read from.
fn as_given<M: Model>(plan: Option<serde_json::Value>) -> Result<M::Plan, EngineError> {
    let plan = plan.ok_or_else(|| EngineError::new("missing_plan", "the request has no plan"))?;
    serde_json::from_value(plan).map_err(|error| {
        EngineError::new(
            "invalid_plan",
            format!("the plan does not match the model: {error}"),
        )
    })
}

/// The changes the plan offers for replanning.
pub fn edits<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let plan = as_given::<M>(request.plan)?;
    out.event("edits", json!({ "edits": M::edits(&plan) }));
    Ok(())
}

/// One change compiled into JSON Patch operations.
pub fn edit<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let wanted = request
        .edit
        .ok_or_else(|| EngineError::new("missing_edit", "the request names no change"))?;
    let plan = as_given::<M>(request.plan)?;
    let compiled = M::edit(&plan, &wanted.id, &wanted.args)?;
    out.event("edit", json!(compiled));
    Ok(())
}

pub fn fill<M: Model, W: Write>(request: Request, out: &mut Output<W>) -> Result<(), EngineError> {
    let (mut plan, issues) = parse::<M>(
        request.plan,
        request.freeze_before.as_deref(),
        request.roads.as_ref(),
    )?;
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
