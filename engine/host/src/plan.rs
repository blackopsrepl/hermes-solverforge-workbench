//! Turning a request's plan and weights into a model plan.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use solverforge::{ConstraintSet, ConstraintWeightError, ConstraintWeights};

use crate::model::{Issue, Model, Severity};
use crate::protocol::EngineError;
use crate::roads::{issues, same, Roads, RoadsConfig, Travel, TravelMode};
use crate::traffic::slowed_periods;

/// Deserializes and prepares the plan. A model that travels gets its travel
/// measured along the roads first, unless the plan brought its own or already
/// carries road travel for every place. Returns the plan with the warnings;
/// any error-level issue refuses the request with every issue attached.
pub fn parse<M: Model>(
    plan: Option<Value>,
    freeze_before: Option<&str>,
    roads: Option<&RoadsConfig>,
) -> Result<(M::Plan, Vec<Issue>), EngineError> {
    let (mut plan, mut issues) = measured::<M>(plan, roads)?;
    issues.extend(M::prepare(&mut plan));
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Error)
        .count();
    if errors > 0 {
        return Err(EngineError::new(
            "invalid_dataset",
            format!("the dataset has {errors} error(s)"),
        )
        .with_detail(json!({ "issues": issues })));
    }
    if let Some(until) = freeze_before {
        let pinned = M::freeze(&mut plan, until)?;
        issues.push(Issue::warning(
            "frozen",
            format!(
                "{pinned} item(s) starting before {until} are pinned to their current assignment"
            ),
        ));
    }
    Ok((plan, issues))
}

/// Deserializes the plan and, for a model that travels, measures its travel
/// along the roads when it needs it. Returns it with what the roads say.
pub fn measured<M: Model>(
    plan: Option<Value>,
    roads: Option<&RoadsConfig>,
) -> Result<(M::Plan, Vec<Issue>), EngineError> {
    let plan = plan.ok_or_else(|| EngineError::new("missing_plan", "the request has no plan"))?;
    let mut plan: M::Plan = serde_json::from_value(plan).map_err(|error| {
        EngineError::new(
            "invalid_plan",
            format!("the plan does not match the model: {error}"),
        )
    })?;
    let issues = road_travel::<M>(&mut plan, roads)?;
    Ok((plan, issues))
}

/// Measures the plan's travel along the roads when it needs it; the roads' issues.
fn road_travel<M: Model>(
    plan: &mut M::Plan,
    roads: Option<&RoadsConfig>,
) -> Result<Vec<Issue>, EngineError> {
    let places = M::places(plan);
    if places.is_empty() {
        return Ok(Vec::new());
    }
    let traffic = M::traffic(plan);
    let mut modes: Vec<TravelMode> = Vec::new();
    for mode in M::travel_modes(plan) {
        if mode != TravelMode::Car && !modes.contains(&mode) {
            modes.push(mode);
        }
    }
    match M::travel(plan) {
        Travel::Given => return Ok(Vec::new()),
        // Kept while it covers every place and was measured with the busy
        // periods the plan has now.
        Travel::Roads(matrix)
            if places
                .iter()
                .all(|place| matrix.locations.iter().any(|l| same(l, &place.point)))
                && matrix
                    .periods
                    .iter()
                    .map(|p| (p.id.as_str(), p.slowdowns.as_slice()))
                    .eq(slowed_periods(&traffic))
                && matrix
                    .modes
                    .iter()
                    .map(|m| m.mode)
                    .eq(modes.iter().copied()) =>
        {
            return Ok(issues(&places, &matrix));
        }
        Travel::Roads(_) | Travel::Missing => {}
    }
    let mut distinct: Vec<crate::roads::Place> = Vec::with_capacity(places.len());
    for place in places {
        if !distinct.iter().any(|p| same(&p.point, &place.point)) {
            distinct.push(place);
        }
    }
    let points: Vec<[f64; 2]> = distinct.iter().map(|p| p.point).collect();
    let (mut matrix, issues) = Roads::load(roads, &points)?.matrix(&distinct, &traffic)?;
    for mode in modes {
        let travel = Roads::load_for(roads, &points, mode)?.mode_travel(&distinct, mode)?;
        matrix.modes.push(travel);
    }
    M::set_roads(plan, matrix);
    Ok(issues)
}

/// Validates the requested multipliers and stores them in the plan.
pub fn apply_weights<M: Model>(
    plan: &mut M::Plan,
    weights: &BTreeMap<String, u32>,
) -> Result<(), EngineError> {
    if weights.is_empty() {
        return Ok(());
    }
    let mut requested = ConstraintWeights::new();
    for (name, multiplier) in weights {
        requested.set(name.clone(), *multiplier);
    }
    let constraints = M::constraints();
    let metadata = constraints.constraint_metadata();
    requested
        .validate(metadata.iter().map(|meta| meta.constraint_ref))
        .map_err(weight_error)?;
    let target = M::weights_mut(plan).ok_or_else(|| {
        EngineError::new(
            "weights_unsupported",
            format!(
                "model {} has no #[constraint_weights] field, so its constraint weights are fixed",
                M::id()
            ),
        )
    })?;
    for (name, multiplier) in weights {
        target.set(name.clone(), *multiplier);
    }
    Ok(())
}

fn weight_error(error: ConstraintWeightError) -> EngineError {
    match error {
        ConstraintWeightError::UnknownConstraint {
            name,
            suggestions,
            known,
        } => EngineError::new(
            "unknown_constraint",
            format!("no constraint is named {name:?}"),
        )
        .with_detail(json!({ "name": name, "suggestions": suggestions, "known": known })),
        ConstraintWeightError::MultiplierTooLarge {
            name,
            multiplier,
            max,
        } => EngineError::new(
            "weight_too_large",
            format!("weight {multiplier} for {name:?} exceeds the maximum {max}"),
        )
        .with_detail(json!({ "name": name, "multiplier": multiplier, "max": max })),
    }
}
