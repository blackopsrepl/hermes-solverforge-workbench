//! Which demos exist and how each is built.

use crate::domain::Plan;

/// Demo ids, default first.
pub fn ids() -> Vec<&'static str> {
    vec!["SMALL_SHOP", "LARGE_WEEK"]
}

/// A prepared demo plan.
pub fn build(id: &str) -> Option<Plan> {
    let dataset = match id {
        "SMALL_SHOP" => super::small::dataset(),
        "LARGE_WEEK" => super::large::dataset(),
        _ => return None,
    };
    let mut plan: Plan = serde_json::from_value(dataset).expect("demo matches the model");
    let issues = crate::prepare::prepare(&mut plan);
    debug_assert!(
        issues
            .iter()
            .all(|i| i.severity != solverforge_workbench_host::Severity::Error),
        "{id}: {issues:?}"
    );
    Some(plan)
}
