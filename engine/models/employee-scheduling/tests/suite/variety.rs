//! Shift assignment and shift variety constraints, each against a
//! hand-built plan with a known penalty.

use crate::common::*;
use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::Plan;

fn with(parts: Value) -> Plan {
    plan(dataset(parts))
}

fn ward_tags() -> Value {
    json!([
        {"id": "ward-a", "type": "ward"},
        {"id": "ward-b", "type": "ward"},
        {"id": "ward-c", "type": "ward"},
    ])
}

fn tagged(id: &str, day: i64, from: i64, to: i64, who: Option<&str>, tags: &[&str]) -> Value {
    shift_with(
        id,
        &at(day, from, 0),
        &at(day, to, 0),
        who,
        json!({"tags": tags}),
    )
}

#[test]
fn preferred_unwanted_and_barred_employees() {
    let plan = with(json!({
        "employees": [employee("ann", json!({"priority": 2})), employee("bob", json!({}))],
        "shifts": [
            shift_with("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"), json!({"preferredEmployees": ["ann"]})),
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), Some("ann"), json!({"unwantedEmployees": ["ann"]})),
            shift_with("c", &at(1, 7, 0), &at(1, 15, 0), Some("bob"), json!({"barredEmployees": ["bob"]})),
        ],
    }));
    assert_eq!(score(&plan, "Preferred employee on shift"), soft(2));
    assert_eq!(score(&plan, "Unwanted employee on shift"), soft(-2));
    assert_eq!(score(&plan, "Barred employee on shift"), hard(-1));
}

#[test]
fn required_and_preferred_shift_tags_of_employees() {
    let plan = with(json!({
        "employees": [employee("ann", json!({
            "requiredShifts": {"withTags": ["ward-a"]},
            "preferredShifts": {"withTags": ["early"]},
        }))],
        "shifts": [
            tagged("s1", 0, 7, 15, Some("ann"), &["ward-a", "early"]),
            tagged("s2", 1, 7, 15, Some("ann"), &["ward-b", "early"]),
            tagged("s3", 2, 15, 23, Some("ann"), &["ward-a", "late"]),
        ],
    }));
    assert_eq!(
        score(&plan, "Shift outside employee's required tags"),
        hard(-1)
    );
    assert_eq!(score(&plan, "Shift has employee's preferred tags"), soft(2));
}

#[test]
fn shift_types_per_period() {
    let plan = with(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "shiftTypes", "period": "week", "categories": ["early", "night"], "max": 2},
            {"kind": "shiftTypes", "period": "week", "level": "preferred", "categories": ["early", "night"], "min": 1, "weight": 2},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c"]})), employee("bob", json!({"contracts": ["c"]}))],
        "shifts": [
            tagged("e0", 0, 7, 15, Some("ann"), &["early"]),
            tagged("e1", 1, 7, 15, Some("ann"), &["early"]),
            tagged("e2", 2, 7, 15, Some("ann"), &["early"]),
        ],
    }));
    assert_eq!(
        score(&plan, "Shift types per period outside required range"),
        hard(-1)
    );
    // ann lacks a night (2); bob lacks an early and a night (4).
    assert_eq!(
        score(&plan, "Shift types per period outside preferred range"),
        soft(-6)
    );
}

#[test]
fn distinct_tags_per_period_for_employee() {
    let plan = with(json!({
        "tags": ward_tags(),
        "contracts": [{"id": "c", "rules": [
            {"kind": "distinctTags", "period": "week", "tagTypes": ["ward"], "max": 1},
            {"kind": "distinctTags", "period": "week", "level": "preferred", "tagTypes": ["ward"], "max": 2},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c"]})), employee("bob", json!({"contracts": ["c"]}))],
        "shifts": [
            tagged("a", 0, 7, 15, Some("ann"), &["ward-a"]),
            tagged("b", 1, 7, 15, Some("ann"), &["ward-b"]),
            tagged("c", 2, 7, 15, Some("ann"), &["ward-c", "early"]),
            tagged("d", 2, 7, 15, Some("bob"), &["ward-a"]),
        ],
    }));
    assert_eq!(
        score(
            &plan,
            "Distinct tags per period for employee outside required range"
        ),
        hard(-2)
    );
    assert_eq!(
        score(
            &plan,
            "Distinct tags per period for employee outside preferred range"
        ),
        soft(-1)
    );
}

#[test]
fn concurrent_shifts_over_limit_across_midnight() {
    let people: Vec<Value> = ["ann", "bob", "cy", "dee", "eve", "fay", "gus", "hal"]
        .iter()
        .map(|id| employee(id, json!({})))
        .collect();
    let plan = with(json!({
        "employees": people,
        "scheduleRules": {"concurrency": [{"max": 2, "withTags": ["icu"]}]},
        "shifts": [
            tagged("a", 0, 7, 15, Some("ann"), &["icu"]),
            tagged("b", 0, 8, 16, Some("bob"), &["icu"]),
            tagged("c", 0, 9, 17, Some("cy"), &["icu"]),
            tagged("d", 0, 10, 12, Some("dee"), &["ward"]),
            tagged("e", 0, 14, 18, Some("eve"), &["icu"]),
            tagged("f", 0, 23, 31, Some("fay"), &["icu"]),
            tagged("g", 1, 6, 10, Some("gus"), &["icu"]),
            shift_with("h", &at(1, 6, 30), &at(1, 8, 0), Some("hal"), json!({"tags": ["icu"]})),
        ],
    }));
    // 09:00 one over, 14:00 two over, Tuesday 06:30 one over.
    assert_eq!(score(&plan, "Concurrent shifts over limit"), hard(-4));
}

#[test]
fn shift_count_per_period_across_employees() {
    let plan = with(json!({
        "employees": [employee("ann", json!({"tags": ["senior"]})), employee("bob", json!({}))],
        "scheduleRules": {"shiftCounts": [
            {"period": "day", "withTags": ["early"], "max": 1},
            {"period": "week", "level": "preferred", "min": 5, "employees": {"withTags": ["senior"]}},
        ]},
        "shifts": [
            tagged("a", 0, 7, 15, Some("ann"), &["early"]),
            tagged("b", 0, 7, 15, Some("bob"), &["early"]),
            tagged("c", 1, 7, 15, Some("ann"), &["early"]),
            tagged("d", 2, 7, 15, None, &["early"]),
        ],
    }));
    assert_eq!(
        score(&plan, "Shift count per period outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Shift count per period outside preferred range"),
        soft(-3)
    );
}

#[test]
fn distinct_tags_per_period_across_schedule() {
    let plan = with(json!({
        "tags": ward_tags(),
        "employees": [employee("ann", json!({})), employee("bob", json!({})), employee("cy", json!({}))],
        "scheduleRules": {"distinctTags": [
            {"period": "week", "tagTypes": ["ward"], "max": 2},
            {"period": "day", "level": "preferred", "tagTypes": ["ward"], "min": 1},
        ]},
        "shifts": [
            tagged("a", 0, 7, 15, Some("ann"), &["ward-a"]),
            tagged("b", 0, 7, 15, Some("bob"), &["ward-b"]),
            tagged("c", 0, 7, 15, Some("cy"), &["ward-c"]),
        ],
    }));
    assert_eq!(
        score(
            &plan,
            "Distinct tags per period across schedule outside required range"
        ),
        hard(-1)
    );
    // Tuesday to Sunday staff no ward at all.
    assert_eq!(
        score(
            &plan,
            "Distinct tags per period across schedule outside preferred range"
        ),
        soft(-6)
    );
}

#[test]
fn schedule_rules_are_validated() {
    let codes = issue_codes(dataset(json!({
        "scheduleRules": {
            "shiftCounts": [{"period": "fortnight", "max": 1}, {"period": "week"}],
            "distinctTags": [{"period": "week", "tagTypes": ["nope"], "max": 1}],
            "concurrency": [{"max": 0}],
        },
    })));
    for code in [
        "unknown_period",
        "rule_limit_missing",
        "unknown_tag_type",
        "rule_negative_limit",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}

#[test]
fn schedule_wide_buckets_score_incrementally_like_in_full() {
    let mut shifts = Vec::new();
    for d in 0..7 {
        for (n, (from, to)) in [(7, 15), (9, 17), (15, 23), (23, 31)].iter().enumerate() {
            let ward = ["ward-a", "ward-b", "ward-c"][(d as usize + n) % 3];
            shifts.push(tagged(
                &format!("s{d}-{n}"),
                d,
                *from,
                *to,
                None,
                &[ward, if n == 3 { "night" } else { "day" }],
            ));
        }
    }
    let plan = with(json!({
        "tags": ward_tags(),
        "contracts": [{"id": "c", "rules": [
            {"kind": "shiftTypes", "period": "week", "categories": ["day", "night"], "max": 4},
            {"kind": "distinctTags", "period": "week", "level": "preferred", "tagTypes": ["ward"], "max": 1},
        ]}],
        "employees": [
            employee("ann", json!({"contracts": ["c"], "tags": ["senior"], "requiredShifts": {"withoutTags": ["night"]}})),
            employee("bob", json!({"contracts": ["c"], "preferredShifts": {"withTags": ["ward-b"]}})),
            employee("cy", json!({})),
            employee("dee", json!({"tags": ["senior"]})),
        ],
        "scheduleRules": {
            "concurrency": [{"max": 1, "withTags": ["ward-a"]}],
            "shiftCounts": [{"period": "day", "max": 3}, {"period": "week", "level": "preferred", "min": 10, "employees": {"withTags": ["senior"]}}],
            "distinctTags": [{"period": "day", "tagTypes": ["ward"], "level": "preferred", "min": 3}],
        },
        "shifts": shifts,
    }));
    assert_incremental_matches_full(plan, 600, 9);
}
