//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::ConstraintInfo;

use crate::constraints::RULES;

/// Every constraint with its group, level and a one-sentence description.
pub fn catalog() -> Vec<ConstraintInfo> {
    RULES
        .iter()
        .map(|rule| ConstraintInfo {
            name: rule.name(),
            group: rule.group(),
            level: rule.level(),
            description: rule.description(),
        })
        .collect()
}
