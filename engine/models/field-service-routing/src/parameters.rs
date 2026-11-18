//! Model parameters: settings a dataset or profile sets under `parameters`.

use serde_json::json;
use solverforge_workbench_host::{priority_options, ParameterInfo, ParameterOption};

use crate::wire::Parameters;

pub fn parameters() -> Vec<ParameterInfo> {
    let d = Parameters::default();
    vec![
        ParameterInfo {
            name: "defaultPriority",
            label: "Default priority",
            default: json!(d.default_priority),
            description: "Priority of a visit that sets none: 1 to 10 are mandatory, opt-1 to opt-10 optional, each level ten times the weight of the next.",
            options: priority_options(),
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "durationAddedForFirstVisitOnLocation",
            label: "Arrival time at a new place",
            kind: "duration",
            unit: "minutes",
            default: json!(d.duration_added_for_first_visit_on_location),
            description: "Minutes added to a visit when the item before it was elsewhere (parking, check-in), for visits that set none.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "visitCompletionRiskMinimalTimeToShiftEnd",
            label: "Risky time before a shift ends",
            kind: "duration",
            unit: "minutes",
            default: json!(d.visit_completion_risk_minimal_time_to_shift_end),
            description: "Minutes before a shift's end within which an important visit risks not being completed.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "visitCompletionRiskMinimalPriority",
            label: "Least priority at risk",
            default: json!(d.visit_completion_risk_minimal_priority),
            description: "Least important priority whose visits carry completion risk.",
            options: priority_options(),
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "defaultTechnicianRating",
            label: "Default technician rating",
            kind: "number",
            unit: "points",
            default: json!(d.default_technician_rating),
            description: "Rating of a technician who has none.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "travelTimeAdjustment",
            label: "Travel time adjustment",
            kind: "object",
            default: json!(d.travel_time_adjustment),
            description: "Every drive takes multiplier times the drive on the roads plus extraTime minutes, for shifts that set none.",
            fields: vec![
                ParameterInfo {
                    name: "multiplier",
                    label: "Multiplier",
                    kind: "number",
                    unit: "×",
                    default: json!(d.travel_time_adjustment.multiplier),
                    description: "Times the drive on the roads.",
                    ..ParameterInfo::default()
                },
                ParameterInfo {
                    name: "extraTime",
                    label: "Extra time",
                    kind: "duration",
                    unit: "minutes",
                    default: json!(d.travel_time_adjustment.extra_time),
                    description: "Minutes added to every drive.",
                    ..ParameterInfo::default()
                },
            ],
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "preferGroupingColocatedVisitsRadiusMeters",
            label: "Same-place radius",
            kind: "number",
            unit: "meters",
            default: json!(d.prefer_grouping_colocated_visits_radius_meters),
            description: "Two consecutive items closer than this many meters count as one place for co-located grouping; at most 200.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "defaultDependencyPoolingType",
            label: "What may run inside a dependency",
            default: json!(d.default_dependency_pooling_type),
            description: "What may run between a preceding visit and the visit waiting on it, for dependencies that set none.",
            options: vec![
                ParameterOption::new("ALLOW_POOLING_ALL", "Anything"),
                ParameterOption::new("ALLOW_POOLING_VISITS", "Only visits outside any dependency"),
                ParameterOption::new("ALLOW_POOLING_DEPENDENCIES", "Only visits of other dependencies"),
                ParameterOption::new("DISALLOW_POOLING", "Nothing: the visit follows right away"),
            ],
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "pinnedStartAllowedDelay",
            label: "Allowed delay of a pinned visit",
            kind: "duration",
            unit: "minutes",
            default: json!(d.pinned_start_allowed_delay),
            description: "Minutes a pinned visit may start after its kept start at a soft cost; later costs hard.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "holidays",
            label: "Holidays",
            kind: "array",
            default: json!(d.holidays),
            description: "Dates that are no working day for dependencies delayed to the next working day.",
            fields: vec![ParameterInfo {
                name: "date",
                label: "Date",
                kind: "date",
                description: "A holiday, YYYY-MM-DD.",
                ..ParameterInfo::default()
            }],
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "pinNextVisitDuringFreeze",
            label: "Freezing pins the next visit",
            kind: "boolean",
            default: json!(d.pin_next_visit_during_freeze),
            description: "Freezing the past also pins the visit each technician is on the way to; a shift may override it.",
            ..ParameterInfo::default()
        },
    ]
}
