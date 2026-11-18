//! The contract a model library meets to run inside the workbench engine.

use serde::de::DeserializeOwned;
use serde::Serialize;
use solverforge::domain::SolutionDescriptor;
use solverforge::{ConstraintSet, ConstraintWeights, PlanningSolution, Solvable, SolverManager};

use serde_json::Value;

use crate::analysis::Analysis;
use crate::edit::{Compiled, Edit};
use crate::fill::{FillAnswer, FillRequest};
use crate::protocol::EngineError;
use crate::roads::{Place, RoadMatrix, Travel};
use crate::view::View;

/// Score type of a model's plan.
pub type ScoreOf<M> = <<M as Model>::Plan as PlanningSolution>::Score;

/// A SolverForge model the engine can describe, solve and evaluate.
///
/// Stock models implement it in this workspace. A model built with
/// solverforge-cli gets a generated implementation that points at its
/// `domain::Plan`, `constraints::create_constraints` and `data` module.
pub trait Model: 'static + Sized {
    /// The planning solution.
    type Plan: Solvable + Serialize + DeserializeOwned;

    /// Stable identifier, for example `employee-scheduling`.
    fn id() -> &'static str;
    /// Display name.
    fn name() -> &'static str;
    /// What the model plans, in one sentence a planner reads first.
    fn summary() -> &'static str {
        ""
    }
    /// Builds the constraint set the plan is scored against.
    fn constraints() -> impl ConstraintSet<Self::Plan, ScoreOf<Self>>;
    /// The model's retained-job manager; one `static` per model.
    fn manager() -> &'static SolverManager<Self::Plan>;
    /// Demo dataset identifiers, default first.
    fn demos() -> Vec<&'static str>;
    /// Builds one demo dataset.
    fn demo(id: &str) -> Option<Self::Plan>;
    /// The planning descriptor the macros generate: `Plan::descriptor()`.
    fn descriptor() -> SolutionDescriptor;

    /// Each demo by id with a name and a one-line summary, default first. The
    /// default names a demo after its id (`TURIN_TWO_DAYS` -> `Turin two days`).
    fn demo_info() -> Vec<DemoInfo> {
        Self::demos()
            .into_iter()
            .map(|id| DemoInfo {
                id,
                name: demo_name(id),
                summary: String::new(),
            })
            .collect()
    }
    /// How a dataset's size reads: which lists of the plan's wire JSON to count
    /// and what to call them. Empty counts every top-level list by its key.
    fn size_fields() -> Vec<SizeField> {
        Vec::new()
    }

    /// Whether the model's plans travel on roads: then [`Model::places`] lists
    /// where, and every request carries the road network cache.
    fn travels() -> bool {
        false
    }
    /// The places a plan travels between, for models whose travel runs on roads.
    /// Empty (the default) for models that do not travel.
    fn places(_plan: &Self::Plan) -> Vec<Place> {
        Vec::new()
    }
    /// What the plan's travel stands on: a matrix it brought, road travel
    /// measured earlier, or nothing yet.
    fn travel(_plan: &Self::Plan) -> Travel {
        Travel::Missing
    }
    /// Gives the plan travel measured along the roads between the places
    /// [`Model::places`] listed, in that order.
    fn set_roads(_plan: &mut Self::Plan, _roads: RoadMatrix) {}

    /// The changes a plan offers for replanning, with each field's choices read
    /// from `plan` (deserialized, not prepared). None by default.
    fn edits(_plan: &Self::Plan) -> Vec<Edit> {
        Vec::new()
    }
    /// Compiles one change offered by [`Model::edits`] into JSON Patch operations.
    fn edit(plan: &Self::Plan, id: &str, _args: &Value) -> Result<Compiled, EngineError> {
        Err(crate::edit::unknown(Self::id(), id, &Self::edits(plan)))
    }

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
    /// Names the record at `index` in the collection of `type_name`. The
    /// default asks [`Model::name_entity`] with the index as the planning id,
    /// which holds for models whose planning ids are collection indexes;
    /// models whose planning ids are not override it.
    fn name_at(plan: &Self::Plan, type_name: &str, index: usize) -> Option<EntityName> {
        Self::name_entity(plan, type_name, &index.to_string())
    }
    /// Model parameters a dataset or profile may set under `parameters`.
    fn parameters() -> Vec<ParameterInfo> {
        Vec::new()
    }
    /// The kind of view [`Model::view`] draws: `"timeline"` or `"routes"`.
    fn view_kind() -> Option<&'static str> {
        None
    }
    /// The plan drawn for people, its broken rules marked from `analysis`.
    fn view(_plan: &Self::Plan, _analysis: &Analysis) -> Option<View> {
        None
    }
    /// Ranked placements for open items, or one applied. The default places
    /// single-value planning variables; models with list variables override it.
    fn fill(
        plan: Self::Plan,
        request: &FillRequest,
    ) -> Result<FillAnswer<Self::Plan>, EngineError> {
        crate::fill::scalar::<Self>(plan, request)
    }
    /// Pins every item that starts before `until` (a local date-time in the
    /// model's wire format) to its current assignment; returns how many were
    /// pinned. Called after [`Model::prepare`].
    fn freeze(_plan: &mut Self::Plan, _until: &str) -> Result<usize, EngineError> {
        Err(EngineError::new(
            "freeze_unsupported",
            format!("model {} has no notion of time to freeze", Self::id()),
        ))
    }
}

/// A demo dataset as a planner picks it.
#[derive(Debug, Clone, Serialize)]
pub struct DemoInfo {
    pub id: &'static str,
    pub name: String,
    pub summary: String,
}

/// One count in a dataset's size: the items at `path` in the plan's wire JSON,
/// a list key or `list.nested` (summed over the outer list), called `label`
/// ("technicians"), or `one` when there is exactly one ("technician").
#[derive(Debug, Clone, Serialize)]
pub struct SizeField {
    pub path: &'static str,
    pub label: &'static str,
    pub one: &'static str,
}

impl SizeField {
    /// How many items sit at this field's path in a plan's wire JSON.
    pub fn count(&self, plan: &serde_json::Value) -> usize {
        let mut parts = self.path.splitn(2, '.');
        let outer = parts.next().unwrap_or_default();
        let Some(items) = plan.get(outer).and_then(|v| v.as_array()) else {
            return 0;
        };
        match parts.next() {
            None => items.len(),
            Some(inner) => items
                .iter()
                .filter_map(|item| item.get(inner).and_then(|v| v.as_array()))
                .map(Vec::len)
                .sum(),
        }
    }
}

/// `TURIN_TWO_DAYS` -> `Turin two days`.
pub fn demo_name(id: &str) -> String {
    let words = id.to_lowercase().replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A model parameter: a setting of the model itself rather than a constraint
/// weight (a travel speed, a planning horizon, a default break length).
#[derive(Debug, Clone, Serialize)]
pub struct ParameterInfo {
    pub name: &'static str,
    /// `integer`, `number`, `boolean`, `string` or `duration` (minutes).
    pub kind: &'static str,
    pub default: serde_json::Value,
    pub description: &'static str,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_demo_is_named_after_its_id() {
        assert_eq!(demo_name("TURIN_TWO_DAYS"), "Turin two days");
        assert_eq!(demo_name(""), "");
    }

    #[test]
    fn a_size_field_counts_a_list_or_a_nested_list() {
        let plan = serde_json::json!({"vehicles": [{"shifts": [1, 2]}, {"shifts": [3]}, {}]});
        let count = |path| {
            SizeField {
                path,
                label: "x",
                one: "x",
            }
            .count(&plan)
        };
        assert_eq!(count("vehicles"), 3);
        assert_eq!(count("vehicles.shifts"), 3);
        assert_eq!(count("visits"), 0);
    }
}
