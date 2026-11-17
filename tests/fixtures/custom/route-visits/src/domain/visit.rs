use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A customer stop a route serves.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Visit {
    #[planning_id]
    pub id: String,
    pub name: String,
    // @solverforge:begin entity-variables
    // @solverforge:end entity-variables
}

impl Visit {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            // @solverforge:begin entity-variable-init
            // @solverforge:end entity-variable-init
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visit_construction() {
        let entity = Visit::new("test-id");
        assert_eq!(entity.id, "test-id");
    }
}
