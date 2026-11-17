use serde::{Deserialize, Serialize};
use solverforge::prelude::*;

/// A crew member who carries out tasks.
#[problem_fact]
#[derive(Serialize, Deserialize)]
pub struct Resource {
    #[planning_id]
    pub id: String,
    pub name: String,
    /// Position in `Plan::resources`, the key a task's `resource_idx` joins on.
    pub slot: usize,
    /// The one trade this crew member works.
    pub skill: String,
}

impl Resource {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            slot: 0,
            skill: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_construction() {
        let fact = Resource::new("test-id", "test");
        assert_eq!(fact.id, "test-id");
        assert_eq!(fact.name, "test");
    }
}
