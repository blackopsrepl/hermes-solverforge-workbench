use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct Nurse {
    #[planning_id]
    pub id: usize,
    pub name: String,
}
