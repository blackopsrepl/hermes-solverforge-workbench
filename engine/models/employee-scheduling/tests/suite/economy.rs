//! Demand, alternatives, cost, activation, fairness and disruption, each
//! against a hand-built plan with a known penalty.

use crate::common::*;
use serde_json::{json, Value};

fn on(id: &str, day: i64, from: i64, to: i64, who: Option<&str>, extra: Value) -> Value {
    shift_with(id, &at(day, from, 0), &at(day, to, 0), who, extra)
}

#[test]
fn demand_slots_are_staffed() {
    let plan = plan(dataset(json!({
        "employees": [employee("ann", json!({})), employee("bob", json!({})), employee("cy", json!({}))],
        "scheduleRules": {"demand": [
            {"slots": [{"from": at(0, 8, 0), "to": at(0, 12, 0), "max": 1}]},
            {"level": "preferred", "slots": [
                {"from": at(0, 8, 0), "to": at(0, 12, 0), "min": 3},
                {"from": at(2, 22, 0), "to": at(3, 2, 0), "min": 1},
                {"from": at(0, 9, 0), "to": at(0, 12, 0), "min": 1},
            ]},
        ]},
        "shifts": [
            on("a", 0, 7, 15, Some("ann"), json!({})),
            on("b", 0, 8, 12, Some("bob"), json!({})),
            on("c", 0, 9, 17, Some("cy"), json!({})),
            on("d", 2, 7, 15, None, json!({})),
        ],
    })));
    assert_eq!(score(&plan, "Required demand over-staffed"), medium(-1));
    assert_eq!(score(&plan, "Required demand under-staffed"), medium(0));
    // One short on Monday morning, and nobody on Wednesday night.
    assert_eq!(score(&plan, "Preferred demand under-staffed"), soft(-2));
    // Three staff for a minimum of one from 09:00.
    assert_eq!(
        score(&plan, "Surplus staff unevenly spread over demand"),
        soft(-4)
    );
}

#[test]
fn alternative_shifts_are_worked_as_often_as_asked() {
    let plan = plan(dataset(json!({
        "alternatives": [
            {"id": "g", "count": 1},
            {"id": "h", "count": 1, "atMost": true},
            {"id": "k", "count": 2},
        ],
        "employees": [employee("ann", json!({})), employee("bob", json!({}))],
        "shifts": [
            on("g1", 0, 7, 15, Some("ann"), json!({"alternativeOf": "g"})),
            on("g2", 0, 9, 17, Some("bob"), json!({"alternativeOf": "g"})),
            on("h1", 1, 7, 15, None, json!({"alternativeOf": "h"})),
            on("k1", 2, 7, 15, None, json!({"alternativeOf": "k"})),
        ],
    })));
    assert_eq!(
        score(&plan, "Alternative shifts worked off count"),
        hard(-3)
    );
    // Open alternatives are not open shifts.
    assert_eq!(score(&plan, "Mandatory shift left open"), medium(0));
}

#[test]
fn contract_cost_with_overtime_tiers() {
    let plan = plan(dataset(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "cost", "period": "week", "level": "preferred", "hourly": 20.0, "baseMinutes": 600,
             "overtime": [{"hourly": 30.0, "minutes": 120}, {"hourly": 40.0}]},
            {"kind": "cost", "period": "week", "hourly": 20.0, "max": 250},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c"]}))],
        "shifts": [
            on("a", 0, 8, 15, Some("ann"), json!({})),
            on("b", 1, 8, 15, Some("ann"), json!({})),
        ],
    })));
    // 600 minutes at 20, 120 at 30, 120 at 40.
    assert_eq!(score(&plan, "Contract cost"), soft(-34000));
    assert_eq!(
        score(&plan, "Contract cost outside required range"),
        hard(-3000)
    );
}

#[test]
fn schedule_cost_activation_and_shares() {
    let plan = plan(dataset(json!({
        "costGroups": [
            {"id": "temp", "activationCost": 50, "fillUp": true, "share": 1},
            {"id": "staff", "share": 1},
        ],
        "scheduleRules": {"costs": [
            {"period": "week", "level": "preferred", "rates": [{"employeeGroup": "temp", "hourly": 30.0}, {"hourly": 20.0}]},
            {"period": "day", "max": 200, "rates": [{"hourly": 20.0}]},
            {"period": "week", "level": "preferred", "max": 600, "rates": [{"hourly": 20.0}]},
        ]},
        "employees": [
            employee("ann", json!({"costGroup": "temp"})),
            employee("ben", json!({"costGroup": "temp"})),
            employee("cy", json!({"costGroup": "staff"})),
            employee("dee", json!({})),
        ],
        "shifts": [
            on("a", 0, 7, 15, Some("ann"), json!({})),
            on("b", 1, 7, 15, Some("ann"), json!({})),
            on("c", 1, 7, 15, Some("ben"), json!({})),
            on("d", 1, 7, 15, Some("dee"), json!({})),
        ],
    })));
    // ann and ben 24h at 30, dee 8h at 20.
    assert_eq!(score(&plan, "Cost per period"), soft(-(72000 + 16000)));
    // 32h at 20 is 64000.
    assert_eq!(
        score(&plan, "Cost per period outside preferred range"),
        soft(-4000)
    );
    // Tuesday: 24h at 20 is 48000, 28000 over.
    assert_eq!(
        score(&plan, "Cost per period outside required range"),
        hard(-28000)
    );
    assert_eq!(score(&plan, "Employee activated"), soft(-100));
    assert_eq!(score(&plan, "Activated employee filled up"), soft(4 + 1));
    // Two temps and no staff activated against equal shares.
    assert_eq!(
        score(&plan, "Activated employees off target share"),
        soft(-2)
    );
}

#[test]
fn fairness_balances_load_with_history() {
    let plan = plan(dataset(json!({
        "employees": [employee("ann", json!({"tags": ["nurse"]})), employee("bob", json!({"tags": ["nurse"]})), employee("cy", json!({}))],
        "scheduleRules": {"fairness": [
            {"balance": "shifts", "employees": {"withTags": ["nurse"]}, "history": {"ann": 2}},
            {"balance": "minutes", "withTags": ["night"]},
        ]},
        "shifts": [
            on("a", 0, 7, 15, Some("ann"), json!({})),
            on("b", 1, 7, 15, Some("cy"), json!({})),
            shift_with("n", &at(1, 22, 0), &at(2, 6, 0), Some("bob"), json!({"tags": ["night"]})),
        ],
    })));
    // Shifts: ann 3² + bob 1²; night minutes: bob 8h².
    assert_eq!(score(&plan, "Uneven shift counts"), soft(-10));
    assert_eq!(score(&plan, "Uneven time worked"), soft(-64));
}

#[test]
fn published_shifts_taken_away() {
    let plan = plan(dataset(json!({
        "tags": [{"id": "icu", "type": "ward"}],
        "contracts": [{"id": "c", "rules": [
            {"kind": "disruption", "level": "preferred", "weight": 5, "withinMinutes": 60, "keepTags": ["icu"]},
            {"kind": "disruption", "withTags": ["icu"]},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c"]})), employee("bob", json!({}))],
        "shifts": [
            on("kept", 0, 7, 15, Some("ann"), json!({"published": "ann"})),
            on("lost", 1, 7, 15, Some("bob"), json!({"published": "ann", "tags": ["icu"]})),
            on("swap", 1, 8, 15, Some("ann"), json!({"tags": ["icu"]})),
            on("gone", 2, 7, 15, None, json!({"published": "ann"})),
            on("far", 2, 12, 20, Some("ann"), json!({})),
        ],
    })));
    assert_eq!(score(&plan, "Published assignment changed"), soft(-2));
    assert_eq!(score(&plan, "Published shifts lost by employee"), soft(-2));
    // ann gains "swap" and "far", bob gains "lost".
    assert_eq!(
        score(&plan, "Unpublished shifts gained by employee"),
        soft(-3)
    );
    assert_eq!(
        score(&plan, "Gained shifts keep tags of lost ones"),
        soft(1)
    );
    // "lost" is made good by "swap", keeping icu; "gone" is not by "far".
    assert_eq!(score(&plan, "Close replacements keep rule tags"), soft(1));
    assert_eq!(
        score(&plan, "Lost shift without preferred close replacement"),
        soft(-5)
    );
    // The required rule has no window, so nothing replaces "lost".
    assert_eq!(
        score(&plan, "Lost shift without required close replacement"),
        hard(-1)
    );
}

#[test]
fn economy_rules_are_validated() {
    let codes = issue_codes(dataset(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "cost", "period": "week", "hourly": 20.0},
            {"kind": "cost", "period": "week", "level": "preferred", "hourly": 20.0, "overtime": [{"hourly": 30.0}]},
        ]}],
        "alternatives": [{"id": "g", "count": -1}],
        "scheduleRules": {
            "demand": [{"slots": [{"from": at(0, 8, 0), "to": at(0, 7, 0), "min": 1}]}],
            "fairness": [{"balance": "shifts", "history": {"zed": 1}}],
            "costs": [{"period": "week", "rates": [{"hourly": 1.0}]}],
        },
        "shifts": [on("s", 0, 7, 15, None, json!({"alternativeOf": "nope", "published": "zed"}))],
    })));
    for code in [
        "rule_limit_missing",
        "overtime_without_base",
        "rule_negative_limit",
        "demand_slot_invalid",
        "unknown_employee",
        "unknown_alternatives",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}

#[test]
fn economy_scores_incrementally_like_in_full() {
    let mut shifts = Vec::new();
    for d in 0..7 {
        for (n, (from, to)) in [(7, 15), (8, 12), (15, 23)].iter().enumerate() {
            let mut extra = json!({"tags": [if n == 2 { "late" } else { "early" }]});
            if n == 1 {
                extra["alternativeOf"] = json!(format!("alt{}", d % 3));
            }
            if d % 2 == 0 {
                extra["published"] = json!(["ann", "bob", "cy"][n]);
            }
            extra["costGroup"] = json!(if n == 2 { "late" } else { "day" });
            shifts.push(on(&format!("s{d}-{n}"), d, *from, *to, None, extra));
        }
    }
    let slots: Vec<Value> = (0..7)
        .map(|d| json!({"from": at(d, 9, 0), "to": at(d, 11, 0), "min": 2, "max": 2}))
        .collect();
    let plan = plan(dataset(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "cost", "period": "week", "level": "preferred", "hourly": 20.0, "baseMinutes": 1200, "overtime": [{"hourly": 30.0}]},
            {"kind": "disruption", "level": "preferred", "withinMinutes": 90},
        ]}],
        "costGroups": [{"id": "temp", "activationCost": 30, "fillUp": true, "share": 1}, {"id": "staff", "share": 2}],
        "alternatives": [{"id": "alt0", "count": 2}, {"id": "alt1", "count": 1, "atMost": true}, {"id": "alt2", "count": 1}],
        "scheduleRules": {
            "demand": [{"slots": slots}],
            "costs": [{"period": "day", "level": "preferred", "max": 400, "rates": [{"shiftGroup": "late", "hourly": 25.0}, {"hourly": 20.0}]}],
            "fairness": [{"balance": "minutes", "history": {"dee": 600}}, {"balance": "shifts", "withTags": ["late"]}],
        },
        "employees": [
            employee("ann", json!({"contracts": ["c"], "costGroup": "staff"})),
            employee("bob", json!({"contracts": ["c"], "costGroup": "staff"})),
            employee("cy", json!({"costGroup": "temp"})),
            employee("dee", json!({"costGroup": "temp"})),
        ],
        "shifts": shifts,
    })));
    assert_incremental_matches_full(plan, 600, 21);
}
