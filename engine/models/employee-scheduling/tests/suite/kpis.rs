//! Indicators of a hand-built plan with known values.

use std::collections::HashMap;

use crate::common::*;
use serde_json::json;
use solverforge_workbench_employee_scheduling::EmployeeScheduling;
use solverforge_workbench_host::Model;

#[test]
fn kpis_of_a_known_plan() {
    let plan = plan(dataset(json!({
        "contracts": [{"id": "paid", "rules": [
            {"kind": "cost", "period": "week", "level": "preferred", "hourly": 20.0, "baseMinutes": 600,
             "overtime": [{"hourly": 30.0}]},
        ]}],
        "alternatives": [{"id": "g", "count": 1, "atMost": true}],
        "scheduleRules": {
            "costs": [{"period": "week", "level": "preferred", "rates": [{"hourly": 10.0}]}],
            "demand": [{"slots": [{"from": at(0, 9, 0), "to": at(0, 11, 0), "min": 3}]}],
        },
        "employees": [
            employee("ann", json!({
                "contracts": ["paid"],
                "preferredTimes": [{"from": at(0, 7, 0), "to": at(0, 15, 0)}],
            })),
            employee("bob", json!({})),
        ],
        "shifts": [
            shift_with("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"), json!({"published": "bob"})),
            shift("b", &at(1, 7, 0), &at(1, 15, 0), Some("ann")),
            shift_with("c", &at(0, 8, 0), &at(0, 12, 0), Some("bob"),
                json!({"optional": true, "unwantedEmployees": ["bob"]})),
            shift("d", &at(2, 7, 0), &at(2, 15, 0), None),
            shift_with("e", &at(3, 7, 0), &at(3, 15, 0), None, json!({"alternativeOf": "g"})),
        ],
    })));
    let kpis: HashMap<&str, f64> = EmployeeScheduling::kpis(&plan)
        .into_iter()
        .map(|kpi| (kpi.id, kpi.value))
        .collect();
    let expected = [
        ("assigned_shifts", 3.0),
        // The open alternative is a choice, not a gap.
        ("unassigned_shifts", 1.0),
        ("assigned_mandatory_shifts", 2.0),
        ("assigned_optional_shifts", 1.0),
        ("activated_employees", 2.0),
        ("total_hours", 20.0),
        ("average_hours", 10.0),
        // ann works 16 hours against a 10-hour base.
        ("overtime_hours", 6.0),
        // ann: 10 h at 20 and 6 h at 30; bob: 4 h at 10.
        ("total_cost", 420.0),
        ("preferences_met", 1.0),
        ("dislikes_hit", 1.0),
        ("travel_distance", 0.0),
        // Hours 16 and 4: 20² / (2 · (16² + 4²)).
        ("fairness", 73.53),
        // Two of three staff from 09:00 to 11:00.
        ("demand_coverage", 66.67),
        ("disruption", 100.0),
    ];
    for (id, value) in expected {
        assert_eq!(kpis.get(id), Some(&value), "{id} in {kpis:?}");
    }
    assert_eq!(kpis.len(), expected.len());
}

#[test]
fn optional_kpis_appear_only_with_their_data() {
    let plan = plan(dataset(json!({
        "employees": [employee("ann", json!({}))],
        "shifts": [shift("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"))],
    })));
    let ids: Vec<&str> = EmployeeScheduling::kpis(&plan)
        .into_iter()
        .map(|kpi| kpi.id)
        .collect();
    assert!(!ids.contains(&"demand_coverage") && !ids.contains(&"disruption"));
}
