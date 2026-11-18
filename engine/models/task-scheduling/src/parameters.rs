//! Model parameters: settings a dataset or profile sets under `parameters`.

use serde_json::json;
use solverforge_workbench_host::ParameterInfo;

use crate::wire::Parameters;

pub fn parameters() -> Vec<ParameterInfo> {
    let defaults = Parameters::default();
    vec![
        ParameterInfo {
            name: "defaultPriority",
            label: "Default priority",
            kind: "integer",
            unit: "points",
            default: json!(defaults.default_priority),
            description: "Priority of a task that sets none; priorities weigh unassigned tasks, ideal ends and resources outside a task's list.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "interruptible",
            label: "Tasks pause over breaks",
            kind: "boolean",
            default: json!(defaults.interruptible),
            description: "Whether a task pauses over its resource's unavailable spans and continues afterwards, unless the task sets noInterruptions; when false, tasks wait for a gap long enough to run whole.",
            ..ParameterInfo::default()
        },
        ParameterInfo {
            name: "brokenLinkPenalty",
            label: "Cost of a broken link",
            kind: "integer",
            unit: "hard points",
            default: json!(defaults.broken_link_penalty),
            description: "Hard cost of one broken structural link: a dependency left unassigned, split across resources, interrupted by other tasks, or handed over on a forbidden transition.",
            ..ParameterInfo::default()
        },
    ]
}
