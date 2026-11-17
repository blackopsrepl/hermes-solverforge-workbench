//! Model parameters: settings a dataset or profile sets under `parameters`.

use serde_json::json;
use solverforge_workbench_host::ParameterInfo;

use crate::wire::Parameters;

pub fn parameters() -> Vec<ParameterInfo> {
    let d = Parameters::default();
    vec![
        ParameterInfo {
            name: "defaultPriority",
            kind: "string",
            default: json!(d.default_priority),
            description: "Priority of a visit that sets none: 1 to 10 are mandatory, opt-1 to opt-10 optional, each level ten times the weight of the next.",
        },
        ParameterInfo {
            name: "durationAddedForFirstVisitOnLocation",
            kind: "duration",
            default: json!(d.duration_added_for_first_visit_on_location),
            description: "Minutes added to a visit when the item before it was elsewhere (parking, check-in), for visits that set none.",
        },
        ParameterInfo {
            name: "visitCompletionRiskMinimalTimeToShiftEnd",
            kind: "duration",
            default: json!(d.visit_completion_risk_minimal_time_to_shift_end),
            description: "Minutes before a shift's end within which an important visit risks not being completed.",
        },
        ParameterInfo {
            name: "visitCompletionRiskMinimalPriority",
            kind: "string",
            default: json!(d.visit_completion_risk_minimal_priority),
            description: "Least important priority whose visits carry completion risk.",
        },
        ParameterInfo {
            name: "defaultTechnicianRating",
            kind: "number",
            default: json!(d.default_technician_rating),
            description: "Rating of a technician who has none.",
        },
        ParameterInfo {
            name: "travelTimeAdjustment",
            kind: "object",
            default: json!(d.travel_time_adjustment),
            description: "Every drive takes multiplier times the plain drive plus extraTime minutes, for shifts that set none.",
        },
        ParameterInfo {
            name: "preferGroupingColocatedVisitsRadiusMeters",
            kind: "number",
            default: json!(d.prefer_grouping_colocated_visits_radius_meters),
            description: "Two consecutive items closer than this many meters count as one place for co-located grouping; at most 200.",
        },
        ParameterInfo {
            name: "defaultDependencyPoolingType",
            kind: "string",
            default: json!(d.default_dependency_pooling_type),
            description: "What may run between a preceding visit and the visit waiting on it, for dependencies that set none.",
        },
        ParameterInfo {
            name: "pinnedStartAllowedDelay",
            kind: "duration",
            default: json!(d.pinned_start_allowed_delay),
            description: "Minutes a pinned visit may start after its kept start at a soft cost; later costs hard.",
        },
        ParameterInfo {
            name: "holidays",
            kind: "array",
            default: json!(d.holidays),
            description: "Dates (YYYY-MM-DD) that are no working day for dependencies delayed to the next working day.",
        },
        ParameterInfo {
            name: "averageSpeedKmh",
            kind: "number",
            default: json!(d.average_speed_kmh),
            description: "Average driving speed for straight-line travel, when the dataset gives no travel matrix.",
        },
        ParameterInfo {
            name: "detourFactor",
            kind: "number",
            default: json!(d.detour_factor),
            description: "Road distance per straight-line distance, for straight-line travel.",
        },
        ParameterInfo {
            name: "pinNextVisitDuringFreeze",
            kind: "boolean",
            default: json!(d.pin_next_visit_during_freeze),
            description: "Freezing the past also pins the visit each technician is on the way to; a shift may override it.",
        },
    ]
}
