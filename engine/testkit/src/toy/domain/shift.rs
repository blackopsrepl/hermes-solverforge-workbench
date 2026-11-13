use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Shift {
    #[planning_id]
    pub id: usize,
    pub day: i64,
    #[planning_pin]
    #[serde(default)]
    pub pinned: bool,
    #[planning_variable(value_range_provider = "nurses", allows_unassigned = true)]
    pub nurse_idx: Option<usize>,
}
