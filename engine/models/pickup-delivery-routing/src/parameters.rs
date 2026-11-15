//! Model parameters: settings a dataset or profile sets under `parameters`.

use serde_json::json;
use solverforge_workbench_host::ParameterInfo;

use crate::wire::Parameters;

pub fn parameters() -> Vec<ParameterInfo> {
    let defaults = Parameters::default();
    vec![
        ParameterInfo {
            name: "defaultPriority",
            kind: "string",
            default: json!(defaults.default_priority),
            description: "Priority of a job that sets none: 1 to 10 are mandatory, opt-1 to opt-10 optional, each level ten times the weight of the next.",
        },
        ParameterInfo {
            name: "maximumTimeBurden",
            kind: "duration",
            default: json!(defaults.maximum_time_burden),
            description: "Minutes pooling may add to a job's ride beyond direct service, for jobs that set none; unset means no limit.",
        },
        ParameterInfo {
            name: "averageSpeedKmh",
            kind: "number",
            default: json!(defaults.average_speed_kmh),
            description: "Average driving speed for straight-line travel, when the dataset gives no travel matrix.",
        },
        ParameterInfo {
            name: "detourFactor",
            kind: "number",
            default: json!(defaults.detour_factor),
            description: "Road distance per straight-line distance, for straight-line travel.",
        },
        ParameterInfo {
            name: "pinNextStopDuringFreeze",
            kind: "boolean",
            default: json!(defaults.pin_next_stop_during_freeze),
            description: "Freezing the past also pins the stop each driver is on the way to; a shift may override it.",
        },
    ]
}
