use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::wire::{Role, Skill};

/// Someone on the shop floor who can be rostered and given manual tasks.
#[problem_fact]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Operator {
    /// Position in the plan's operator list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: Role,
    /// Never rostered on nights.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub day_only: bool,
    /// Skills beyond the ones the role brings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<Skill>,
    /// Roster days (`YYYY-MM-DD`) the operator cannot work: leave, training.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unavailable: Vec<String>,
}

impl Operator {
    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}
