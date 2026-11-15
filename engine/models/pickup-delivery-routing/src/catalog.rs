//! What every constraint means, in the workbench's words.

use solverforge_workbench_host::ConstraintInfo;

use crate::constraints::RULES;

/// Every constraint with its group, level and a one-sentence description.
pub fn catalog() -> Vec<ConstraintInfo> {
    RULES
        .iter()
        .map(|&(name, group, level, description)| ConstraintInfo {
            name,
            group,
            level,
            description,
        })
        .collect()
}
