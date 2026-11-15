//! Workshop bays and technician crews.

use serde::{Deserialize, Serialize};

use super::common::{is_one, is_zero, one, Span};

/// A hangar, dock or workshop bay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Bay {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Kind of bay (`hangar`, `paint`, `engine`…), matched by tasks' `bayClasses`.
    pub class: String,
    /// Fleet types the bay holds; every type when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fleets: Vec<String>,
    /// Units the bay holds at once; 1 by default.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub capacity: i64,
    /// What one day of use costs, in currency units.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cost_per_day: i64,
    /// Days the bay is closed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub closures: Vec<Span>,
}

impl Bay {
    pub fn new(id: impl Into<String>, class: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            class: class.into(),
            fleets: Vec::new(),
            capacity: 1,
            cost_per_day: 0,
            closures: Vec::new(),
        }
    }
}

/// Technicians of one trade or licence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Crew {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Technicians on regular time each day.
    pub technicians: i64,
    /// Extra technicians overtime can add each day.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub overtime: i64,
    /// Days the regular strength differs (leave, courses, holidays).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<CrewChange>,
}

/// The regular strength of a crew over an inclusive range of days.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrewChange {
    pub from: String,
    pub to: String,
    pub technicians: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
