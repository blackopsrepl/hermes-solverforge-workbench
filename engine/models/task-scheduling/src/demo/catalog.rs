//! The demo datasets by id.

use crate::domain::Plan;
use crate::prepare::prepare;

/// Demo ids.
pub fn ids() -> Vec<&'static str> {
    vec!["FACTORY"]
}

/// Builds and prepares demo `id`.
pub fn build(id: &str) -> Option<Plan> {
    let mut plan = match id {
        "FACTORY" => super::large::factory(),
        _ => return None,
    };
    let issues = prepare(&mut plan);
    debug_assert!(
        issues
            .iter()
            .all(|issue| issue.severity != solverforge_workbench_host::Severity::Error),
        "demo {id} has errors: {issues:?}"
    );
    Some(plan)
}
