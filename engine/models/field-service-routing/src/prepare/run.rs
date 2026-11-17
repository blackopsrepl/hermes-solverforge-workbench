use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::build::build;
use super::place::place;
use crate::constraints::RULES;
use crate::domain::Plan;

/// Checks the dataset, derives its facts and lays its itineraries into the
/// shifts' lists. Errors refuse the dataset; warnings go with the result.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    let mut issues = Vec::new();
    let Some(built) = build(plan, &mut issues) else {
        return issues;
    };
    let (facts, elements, shifts) = built;
    plan.facts = Arc::new(facts);
    plan.elements = elements;
    plan.shifts = shifts;
    place(plan, &mut issues);
    // Rules off by default stay off unless a dataset or profile weighs them.
    for rule in RULES.iter().filter(|r| r.off_by_default()) {
        if plan.weights.get(rule.name()).is_none() {
            plan.weights.set(rule.name(), 0);
        }
    }
    issues
}
