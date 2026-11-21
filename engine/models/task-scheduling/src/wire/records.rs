//! Dataset records: tags, types, spans, assignments, dependencies, transitions.

use serde::{Deserialize, Serialize};

/// Units of a part that are in stock from the start, or arrive at `at`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StockArrival {
    pub part: String,
    pub quantity: i64,
    /// When it arrives; on hand from the start when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

/// A capability a resource carries and a task can require: a machine
/// capability (`CNC 5-axis`) or an employee skill (`Welding`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// A task type (what changeovers depend on) or a machine type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeDef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Whether a resource is a machine or a person.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    #[default]
    Machine,
    Employee,
}

/// A tag a resource carries, always or only inside one time span.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagRef {
    pub tag: String,
    /// Local date-time from which the tag holds; unset means always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Local date-time until which the tag holds (exclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
}

/// A time span a resource cannot work: maintenance, a holiday, a break.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: String,
    pub end: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A task placed on a resource, in processing order. `start` is written by
/// the solver; on input it is the start a pinned task keeps.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assignment {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

/// How long a task takes on one resource, overriding its default duration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurationOn {
    pub resource: String,
    /// Minutes.
    pub duration: i64,
}

/// The point of the preceding task a dependency's offsets count from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    #[default]
    End,
    Start,
}

/// A task that must run before the task that declares it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub task: String,
    #[serde(default, skip_serializing_if = "is_end")]
    pub anchor: Anchor,
    /// Least minutes from the anchor to this task's start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_offset: Option<i64>,
    /// Most minutes from the anchor to this task's start; unset is unbounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_offset: Option<i64>,
    /// Both tasks must run on the same resource.
    #[serde(default, skip_serializing_if = "is_false")]
    pub same_resource: bool,
    /// No other task may run between the two on their resource.
    #[serde(default, skip_serializing_if = "is_false")]
    pub no_intermediate_tasks: bool,
}

/// Setup time on a machine type when it switches from one task type to
/// another (the same type twice is a cleaning or a tool check).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changeover {
    pub machine_type: String,
    pub from: String,
    pub to: String,
    /// Minutes.
    pub duration: i64,
}

/// A preferred or forbidden hand-over of dependent tasks between two
/// resources: the preceding task on `from`, the dependent one on `to`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTransition {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward: Option<i64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub forbidden: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_end(anchor: &Anchor) -> bool {
    *anchor == Anchor::End
}
