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
