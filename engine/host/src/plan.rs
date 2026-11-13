//! Turning a request's plan and weights into a model plan.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use solverforge::{ConstraintSet, ConstraintWeightError, ConstraintWeights};

use crate::model::Model;
use crate::protocol::EngineError;

/// Deserializes the plan, reporting the JSON path of the first bad field.
pub fn parse<M: Model>(plan: Option<Value>) -> Result<M::Plan, EngineError> {
    let plan = plan.ok_or_else(|| EngineError::new("missing_plan", "the request has no plan"))?;
    serde_json::from_value(plan).map_err(|error| {
        EngineError::new(
            "invalid_plan",
            format!("the plan does not match the model: {error}"),
        )
    })
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
