//! The demos at scale: every constraint reachable, incremental scoring exact.

use std::collections::BTreeSet;

use crate::common::*;
use solverforge_workbench_employee_scheduling::{EmployeeScheduling, Plan};
use solverforge_workbench_host::Model;

/// The large demo with movable shifts given to employees by `pick` (which
/// sees the shift's position and the staff count) or left open.
fn assigned(pick: impl Fn(usize, usize) -> Option<usize>) -> Plan {
    let mut plan = EmployeeScheduling::demo("LARGE_MONTH").expect("large demo");
    let employees = plan.employees.len();
    for (n, shift) in plan
        .shifts
        .iter_mut()
        .enumerate()
        .filter(|(_, s)| !s.pinned)
    {
        shift.employee_idx = pick(n, employees);
    }
    plan
}

/// The large demo with every movable shift given to a seeded pseudo-random
/// employee (or left open), so every family of rules has work to score.
fn scrambled(seed: u64) -> Plan {
    let mut plan = EmployeeScheduling::demo("LARGE_MONTH").expect("large demo");
    let employees = plan.employees.len();
    let mut state = seed;
    for shift in plan.shifts.iter_mut().filter(|s| !s.pinned) {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let n = ((state >> 33) as usize) % (employees + 3);
        shift.employee_idx = (n < employees).then_some(n);
    }
    plan
}

#[test]
fn the_large_demo_exercises_every_constraint() {
    let mut silent: BTreeSet<String> = EmployeeScheduling::catalog()
        .into_iter()
        .map(|info| info.name.to_owned())
        .collect();
    // Random rosters, everything crowded onto a few people of each contract,
    // and nothing assigned at all: maximum rules need crowding, minimum
    // rules need emptiness.
    let mut plans: Vec<Plan> = [1, 2, 3].into_iter().map(scrambled).collect();
    for first in [0, 25, 40] {
        plans.push(assigned(|n, _| Some(first + n % 3)));
    }
    plans.push(assigned(|_, _| None));
    // Pinned shifts taken away too, as a replan that ignores promises would.
    let mut unpinned = assigned(|_, _| None);
    unpinned
        .shifts
        .iter_mut()
        .for_each(|s| s.employee_idx = None);
    plans.push(unpinned);
    for plan in &plans {
        for (name, score) in scores(plan) {
            if score != Score::ZERO {
                silent.remove(&name);
            }
        }
    }
    assert!(silent.is_empty(), "never scored: {silent:?}");
}

#[test]
fn the_large_demo_scores_incrementally_like_in_full() {
    assert_incremental_matches_full(scrambled(4), 300, 77);
}

#[test]
fn the_large_demos_published_week_is_one_a_plan_could_have_made() {
    let open = assigned(|_, _| None);
    let mut published = open.clone();
    let index: std::collections::HashMap<String, usize> = published
        .employees
        .iter()
        .enumerate()
        .map(|(n, employee)| (employee.id.clone(), n))
        .collect();
    let mut count = 0;
    for shift in published.shifts.iter_mut().filter(|s| !s.pinned) {
        if let Some(employee) = shift.published.as_ref() {
            shift.employee_idx = Some(index[employee]);
            count += 1;
        }
    }
    // Every fourth shift of the first week, less any no one could take.
    assert!(count >= 30, "only {count} shifts were published");
    // Giving every published shift to its employee breaks no hard rule that
    // leaving them open does not already break.
    assert!(
        total(&published).hard() >= total(&open).hard(),
        "published {} vs open {}",
        total(&published),
        total(&open)
    );
}
