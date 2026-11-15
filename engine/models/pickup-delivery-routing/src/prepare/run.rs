use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::build::build;
use super::place::place;
use crate::constraints::WAITING;
use crate::domain::Plan;

/// Checks the dataset, derives its facts and lays its itineraries into the
/// shifts' lists. Errors refuse the dataset; warnings go with the result.
pub fn prepare(plan: &mut Plan) -> Vec<Issue> {
    let mut issues = Vec::new();
    let Some(built) = build(plan, &mut issues) else {
        return issues;
    };
    let (facts, visits, shifts) = built;
    plan.facts = Arc::new(facts);
    plan.visits = visits;
    plan.shifts = shifts;
    place(plan, &mut issues);
    // Waiting time is off unless a dataset or profile weighs it.
    if plan.weights.get(WAITING).is_none() {
        plan.weights.set(WAITING, 0);
    }
    issues
}
