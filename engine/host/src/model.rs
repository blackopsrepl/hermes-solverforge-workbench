//! The contract a model library meets to run inside the workbench engine.

use serde::de::DeserializeOwned;
use serde::Serialize;
use solverforge::{ConstraintSet, ConstraintWeights, PlanningSolution, Solvable, SolverManager};

/// Score type of a model's plan.
pub type ScoreOf<M> = <<M as Model>::Plan as PlanningSolution>::Score;

/// A SolverForge model the engine can describe, solve and evaluate.
///
/// Stock models implement it in this workspace. A model built with
/// solverforge-cli gets a generated implementation that points at its
/// `domain::Plan`, `constraints::create_constraints` and `data` module.
pub trait Model: 'static {
    /// The planning solution.
    type Plan: Solvable + Serialize + DeserializeOwned;

    /// Stable identifier, for example `employee-scheduling`.
    fn id() -> &'static str;
    /// Display name.
    fn name() -> &'static str;
    /// Builds the constraint set the plan is scored against.
    fn constraints() -> impl ConstraintSet<Self::Plan, ScoreOf<Self>>;
    /// The model's retained-job manager; one `static` per model.
    fn manager() -> &'static SolverManager<Self::Plan>;
    /// Demo dataset identifiers, default first.
    fn demos() -> Vec<&'static str>;
    /// Builds one demo dataset.
    fn demo(id: &str) -> Option<Self::Plan>;

    /// Checks a plan and fills its derived fields before it is scored or solved.
    ///
    /// Any issue of severity `Error` refuses the request; warnings are reported
    /// with the result.
    fn prepare(_plan: &mut Self::Plan) -> Vec<Issue> {
        Vec::new()
    }
    /// The plan as JSON for the caller. Models whose wire format names related
    /// records by id instead of by index override this.
    fn present(plan: &Self::Plan) -> serde_json::Value {
        serde_json::to_value(plan).unwrap_or_default()
    }
    /// The plan's `#[constraint_weights]` field, when the model has one.
    fn weights_mut(_plan: &mut Self::Plan) -> Option<&mut ConstraintWeights> {
        None
    }
    /// Descriptions, groups, levels and defaults of the model's constraints.
    fn catalog() -> Vec<ConstraintInfo> {
        Vec::new()
    }
    /// Key performance indicators of a plan.
    fn kpis(_plan: &Self::Plan) -> Vec<Kpi> {
        Vec::new()
    }
    /// Names an entity a constraint match blames, found by its type name
    /// (`Shift`) and its planning id as analysis renders it: the record's id
    /// in the model's wire format and a label people read. Called only for
    /// entities with a planning id; `None` reports the entity's `Debug` text.
    fn name_entity(_plan: &Self::Plan, _type_name: &str, _planning_id: &str) -> Option<EntityName> {
        None
    }
}

/// An entity named in a constraint match, as the caller shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EntityName {
    /// The record's id in the model's wire format, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub label: String,
}

/// Catalog entry for one constraint, keyed by its full name.
#[derive(Debug, Clone, Serialize)]
pub struct ConstraintInfo {
    pub name: &'static str,
    pub group: &'static str,
    pub level: Level,
    pub description: &'static str,
}

/// Score level a constraint penalizes or rewards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Hard,
    Medium,
    Soft,
}

/// One key performance indicator of a plan.
#[derive(Debug, Clone, Serialize)]
pub struct Kpi {
    pub id: &'static str,
    pub label: &'static str,
    pub value: f64,
    pub unit: &'static str,
}

/// A problem found in a plan before it is scored.
#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

impl Issue {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}
