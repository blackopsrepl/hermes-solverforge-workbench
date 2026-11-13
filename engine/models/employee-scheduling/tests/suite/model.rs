//! The model's contract with the workbench host: catalog, demos and the
//! round trip through the wire format.

use std::collections::BTreeSet;

use crate::common::*;
use serde_json::json;
use solverforge::ConstraintSet;
use solverforge_workbench_employee_scheduling::{EmployeeScheduling, Plan};
use solverforge_workbench_host::{Model, Severity};

#[test]
fn the_catalog_lists_exactly_the_constraints() {
    let constraints = EmployeeScheduling::constraints();
    let declared: BTreeSet<String> = constraints
        .constraint_metadata()
        .iter()
        .map(|meta| meta.constraint_ref.full_name())
        .collect();
    let catalog = EmployeeScheduling::catalog();
    let described: BTreeSet<String> = catalog.iter().map(|info| info.name.to_owned()).collect();
    assert_eq!(
        declared.len(),
        constraints.constraint_metadata().len(),
        "names are unique"
    );
    assert_eq!(described.len(), catalog.len(), "catalog names are unique");
    assert_eq!(declared, described);
    for info in &catalog {
        assert!(
            info.description.ends_with('.'),
            "{} needs a sentence",
            info.name
        );
    }
}

#[test]
fn every_demo_round_trips_through_its_wire_format() {
    for id in EmployeeScheduling::demos() {
        let plan = EmployeeScheduling::demo(id).expect("listed demo builds");
        let json = EmployeeScheduling::present(&plan);
        let mut again: Plan = serde_json::from_value(json.clone()).expect("demo output reads back");
        let issues = EmployeeScheduling::prepare(&mut again);
        assert!(
            issues.iter().all(|issue| issue.severity != Severity::Error),
            "{id}: {issues:?}"
        );
        assert_eq!(
            EmployeeScheduling::present(&again),
            json,
            "{id} reads back unchanged"
        );
        // Only deliberately pinned shifts start assigned.
        for shift in &plan.shifts {
            assert_eq!(
                shift.employee_idx.is_some(),
                shift.pinned,
                "{id} shift {}",
                shift.id
            );
        }
    }
}

#[test]
fn present_writes_assignments_back_by_id() {
    let mut plan = plan(dataset(json!({
        "employees": [employee("ann", json!({})), employee("bob", json!({}))],
        "shifts": [shift("a", &at(0, 7, 0), &at(0, 15, 0), None)],
    })));
    plan.shifts[0].employee_idx = Some(1);
    let json = EmployeeScheduling::present(&plan);
    assert_eq!(json["shifts"][0]["employee"], "bob");
    assert_eq!(json["shifts"][0]["start"], "2027-03-01T07:00");
}
