use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A job one crew member carries out.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Task {
    #[planning_id]
    pub id: String,
    pub name: String,
    /// The trade the job needs.
    pub skill: String,
    pub hours: i64,
    // @solverforge:begin entity-variables
    #[planning_variable(value_range_provider = "resources", allows_unassigned = true)]
    pub resource_idx: Option<usize>,
    // @solverforge:end entity-variables
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            skill: String::new(),
            hours: 1,
            // @solverforge:begin entity-variable-init
            resource_idx: None,
            // @solverforge:end entity-variable-init
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_construction() {
        let entity = Task::new("test-id");
        assert_eq!(entity.id, "test-id");
    }
}
