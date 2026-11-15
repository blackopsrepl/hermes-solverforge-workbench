//! The pieces of a maintenance task record.

use serde::{Deserialize, Serialize};

/// Technicians of one crew a task needs on each of its days.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Labour {
    pub crew: String,
    pub technicians: i64,
}

/// Parts a task consumes on its first day.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PartNeed {
    pub part: String,
    pub quantity: i64,
}

/// A bay and a first day.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Placement {
    pub bay: String,
    pub start: String,
}
