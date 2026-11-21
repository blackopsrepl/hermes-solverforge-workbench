use std::sync::Arc;

use solverforge_workbench_host::Issue;

use super::build::build;
use super::place::place;
use crate::constraints::{DRIVER_BALANCE, DRIVER_USED, WAITING};
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
    // Waiting time, drivers used and balance between drivers are off unless
    // a dataset or profile weighs them.
    for rule in [WAITING, DRIVER_USED, DRIVER_BALANCE] {
        if plan.weights.get(rule).is_none() {
            plan.weights.set(rule, 0);
        }
    }
    issues
}
