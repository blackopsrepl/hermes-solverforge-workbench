//! Model settings a dataset or profile may set under `parameters`.

use serde::{Deserialize, Serialize};

use super::records::{PoolingType, TravelTimeAdjustment};

/// Model settings a dataset or profile may set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Parameters {
    /// Priority of a visit that sets none.
    pub default_priority: String,
    /// Minutes added to a visit when the visit before it was elsewhere, for
    /// visits that set none.
    pub duration_added_for_first_visit_on_location: i64,
    /// Visits ending this many minutes or more before their shift's end
    /// carry no completion risk.
    pub visit_completion_risk_minimal_time_to_shift_end: i64,
    /// Only visits of this priority or a more important one carry
    /// completion risk.
    pub visit_completion_risk_minimal_priority: String,
    /// Rating of a technician who has none.
    pub default_technician_rating: f64,
    /// Scales and pads every drive, for shifts that set none.
    pub travel_time_adjustment: TravelTimeAdjustment,
    /// Places closer than this count as one place for co-located grouping,
    /// for shifts that set none; at most 200.
    pub prefer_grouping_colocated_visits_radius_meters: i64,
    /// What may run between a preceding visit and the visit waiting on it,
    /// for dependencies that set none.
    pub default_dependency_pooling_type: PoolingType,
    /// Minutes a pinned visit may start after its kept start at a soft
    /// cost; later costs hard.
    pub pinned_start_allowed_delay: i64,
    /// Dates (`YYYY-MM-DD`) that are no working day for `NEXT_WORKING_DAY`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub holidays: Vec<String>,
    /// Straight-line travel: average driving speed.
    pub average_speed_kmh: f64,
    /// Straight-line travel: road distance per straight-line distance.
    pub detour_factor: f64,
    /// Freezing also pins the visit a technician is on the way to.
    pub pin_next_visit_during_freeze: bool,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            default_priority: "6".into(),
            duration_added_for_first_visit_on_location: 0,
            visit_completion_risk_minimal_time_to_shift_end: 0,
            visit_completion_risk_minimal_priority: "6".into(),
            default_technician_rating: 0.0,
            travel_time_adjustment: TravelTimeAdjustment::default(),
            prefer_grouping_colocated_visits_radius_meters: 0,
            default_dependency_pooling_type: PoolingType::AllowPoolingAll,
            pinned_start_allowed_delay: 0,
            holidays: Vec::new(),
            average_speed_kmh: 32.0,
            detour_factor: 1.35,
            pin_next_visit_during_freeze: true,
        }
    }
}
