//! Model settings a dataset or profile may set under `parameters`.

use serde::{Deserialize, Serialize};

/// Model settings a dataset or profile may set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Parameters {
    /// Priority of a job that sets none.
    pub default_priority: String,
    /// Minutes pooling may add to a job's ride, for jobs that set none;
    /// unset means no limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_time_burden: Option<i64>,
    /// Straight-line travel: average driving speed.
    pub average_speed_kmh: f64,
    /// Straight-line travel: road distance per straight-line distance.
    pub detour_factor: f64,
    /// Freezing also pins the stop a driver is on the way to.
    pub pin_next_stop_during_freeze: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            default_priority: "6".into(),
            maximum_time_burden: None,
            average_speed_kmh: 32.0,
            detour_factor: 1.35,
            pin_next_stop_during_freeze: true,
        }
    }
}
