//! Model settings a dataset or profile may set under `parameters`.

use serde::{Deserialize, Serialize};

/// Model settings a dataset or profile may set.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Parameters {
    /// Priority of a task that sets none.
    pub default_priority: i64,
    /// Whether a task pauses over its resource's unavailable spans (and
    /// continues afterwards) unless it says otherwise. When false, tasks wait
    /// for a gap long enough to run without interruption.
    pub interruptible: bool,
    /// Hard cost of a broken structural link: a dependency left unassigned,
    /// split across resources, interrupted by other tasks, or handed over on
    /// a forbidden transition.
    pub broken_link_penalty: i64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            default_priority: 1,
            interruptible: true,
            broken_link_penalty: 1_000,
        }
    }
}
