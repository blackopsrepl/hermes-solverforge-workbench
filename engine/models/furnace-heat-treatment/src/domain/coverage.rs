use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

use crate::wire::{Role, ShiftKind};

/// How many operators of a role one shift needs.
#[problem_fact]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    /// Position in the plan's coverage list; set by `prepare`.
    #[planning_id]
    #[serde(skip)]
    pub index: usize,
    /// `<day>/<shift>/<role>` unless the dataset names it.
    #[serde(default)]
    pub id: String,
    /// Roster day, `YYYY-MM-DD`.
    pub day: String,
    pub shift: ShiftKind,
    pub role: Role,
    pub count: i64,
    /// Dense shift number; set by `prepare`.
    #[serde(skip)]
    pub shift_number: usize,
}
