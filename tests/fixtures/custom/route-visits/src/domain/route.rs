use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// One van's round: the visits it makes, in order.
#[planning_entity]
#[derive(Serialize, Deserialize)]
pub struct Route {
    #[planning_id]
    pub id: String,
    pub name: String,
    /// Most visits the van makes in a round.
    pub capacity: usize,
    // @solverforge:begin entity-variables
    #[planning_list_variable(element_collection = "visits")]
    pub visits: Vec<usize>,
    // @solverforge:end entity-variables
}

impl Route {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            capacity: 0,
            // @solverforge:begin entity-variable-init
            visits: Vec::new(),
            // @solverforge:end entity-variable-init
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_construction() {
        let entity = Route::new("test-id");
        assert_eq!(entity.id, "test-id");
    }
}
