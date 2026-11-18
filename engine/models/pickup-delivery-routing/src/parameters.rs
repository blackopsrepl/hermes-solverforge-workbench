//! Model parameters: settings a dataset or profile sets under `parameters`.

use serde_json::json;
use solverforge_workbench_host::{priority_options, ParameterInfo};

use crate::wire::Parameters;

pub fn parameters() -> Vec<ParameterInfo> {
    let defaults = Parameters::default();
    vec![
        ParameterInfo {
            name: "defaultPriority",
            label: "Default priority",
            default: json!(defaults.default_priority),
            description: "Priority of a job that sets none: 1 to 10 are mandatory, opt-1 to opt-10 optional, each level ten times the weight of the next.",
            options: priority_options(),
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "maximumTimeBurden",
            label: "Longest added ride time",
            kind: "duration",
            unit: "minutes",
            default: json!(defaults.maximum_time_burden),
            description: "Minutes pooling may add to a job's ride beyond direct service, for jobs that set none; unset means no limit.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "pinNextStopDuringFreeze",
            label: "Freezing pins the next stop",
            kind: "boolean",
            default: json!(defaults.pin_next_stop_during_freeze),
            description: "Freezing the past also pins the stop each driver is on the way to; a shift may override it.",
            ..ParameterInfo::default()
        },
    ]
}
