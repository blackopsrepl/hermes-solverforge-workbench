//! Staffing, skills, risk and availability constraints, each against a
//! hand-built plan with a known penalty.

use crate::common::*;
use serde_json::json;

fn two_people() -> Vec<serde_json::Value> {
    vec![
        employee(
            "ann",
            json!({"skills": ["nurse", {"id": "triage", "level": 2}]}),
        ),
        employee(
            "bob",
            json!({"skills": [{"id": "nurse", "from": "2027-03-03T00:00"}]}),
        ),
    ]
}

#[test]
fn open_mandatory_shifts_cost_their_importance_in_medium() {
    let plan = plan(dataset(json!({
        "employees": two_people(),
        "shifts": [
            shift("a", &at(0, 7, 0), &at(0, 15, 0), None),
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), None, json!({"importance": 5})),
            shift("c", &at(2, 7, 0), &at(2, 15, 0), Some("ann")),
            // Before the schedule: history, never counted as open.
            shift_with("h", &at(-1, 7, 0), &at(-1, 15, 0), None, json!({"pinned": true})),
        ],
    })));
    assert_eq!(score(&plan, "Mandatory shift left open"), medium(-6));
    assert_eq!(score(&plan, "Optional shift left open"), soft(0));
}

#[test]
fn open_optional_shifts_cost_their_importance_in_soft() {
    let plan = plan(dataset(json!({
        "employees": two_people(),
        "shifts": [
            shift_with("a", &at(0, 7, 0), &at(0, 15, 0), None, json!({"optional": true, "importance": 3})),
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), Some("bob"), json!({"optional": true})),
        ],
    })));
    assert_eq!(score(&plan, "Optional shift left open"), soft(-3));
    assert_eq!(score(&plan, "Mandatory shift left open"), medium(0));
}

#[test]
fn required_skills_count_level_and_validity() {
    let plan = plan(dataset(json!({
        "employees": two_people(),
        "shifts": [
            // ann holds nurse and triage level 2: fine.
            shift_with("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"),
                json!({"requiredSkills": ["nurse", {"id": "triage", "level": 2}]})),
            // ann's triage is below level 3: one missing.
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), Some("ann"),
                json!({"requiredSkills": [{"id": "triage", "level": 3}]})),
            // bob's nurse skill is not valid yet on Monday: one missing; triage too.
            shift_with("c", &at(0, 7, 0), &at(0, 15, 0), Some("bob"),
                json!({"requiredSkills": ["nurse", "triage"]})),
            // Any one skill is enough: bob has nurse by Thursday.
            shift_with("d", &at(3, 7, 0), &at(3, 15, 0), Some("bob"),
                json!({"requiredSkills": ["nurse", "triage"], "anyRequiredSkill": true})),
            // Any one skill, none held: counts once.
            shift_with("e", &at(0, 16, 0), &at(0, 20, 0), Some("bob"),
                json!({"requiredSkills": ["nurse", "triage"], "anyRequiredSkill": true})),
        ],
    })));
    assert_eq!(score(&plan, "Required skill lacking"), hard(-4));
}

#[test]
fn preferred_skills_and_risks() {
    let plan = plan(dataset(json!({
        "employees": [
            employee("ann", json!({"skills": ["nurse"], "avoidRisks": ["radiation", "lifting"]})),
            employee("bob", json!({"priority": 2})),
        ],
        "shifts": [
            shift_with("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"),
                json!({"preferredSkills": ["nurse", "triage"], "risks": ["radiation", "noise", "lifting"]})),
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), Some("bob"),
                json!({"preferredSkills": ["nurse"], "risks": ["radiation"]})),
        ],
    })));
    // ann misses triage (1); bob misses nurse (1) times priority 2.
    assert_eq!(score(&plan, "Preferred skill lacking"), soft(-3));
    assert_eq!(score(&plan, "Prohibited risk exposure"), hard(-2));
}

#[test]
fn unavailable_and_available_only_minutes() {
    let plan = plan(dataset(json!({
        "employees": [
            employee("ann", json!({"unavailable": [
                {"from": at(0, 12, 0), "to": at(0, 18, 0)},
                {"from": at(1, 0, 0), "to": at(2, 0, 0), "withTags": ["night"]},
            ]})),
            employee("bob", json!({"availableOnly": [
                {"from": at(0, 6, 0), "to": at(0, 10, 0)},
                {"from": at(0, 9, 0), "to": at(0, 12, 0)},
            ]})),
        ],
        "shifts": [
            // 3 h inside ann's Monday block.
            shift("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann")),
            // Tuesday day shift: the Tuesday block only covers night shifts.
            shift_with("b", &at(1, 7, 0), &at(1, 15, 0), Some("ann"), json!({"tags": ["day"]})),
            // Tuesday night shift, one hour inside the block.
            shift_with("c", &at(1, 23, 0), &at(2, 7, 0), Some("ann"), json!({"tags": ["night"]})),
            // bob is available 06:00-12:00 Monday: 07-15 has 3 h outside.
            shift("d", &at(0, 7, 0), &at(0, 15, 0), Some("bob")),
        ],
    })));
    assert_eq!(score(&plan, "Works while unavailable"), hard(-(180 + 60)));
    assert_eq!(score(&plan, "Works outside available hours"), hard(-180));
}

#[test]
fn preferred_and_unwanted_minutes_are_weighted() {
    let plan = plan(dataset(json!({
        "employees": [
            employee("ann", json!({
                "priority": 2,
                "preferredTimes": [{"from": at(0, 6, 0), "to": at(0, 10, 0), "weight": 3}],
                "unwantedTimes": [{"from": at(0, 14, 0), "to": at(0, 20, 0)}],
            })),
        ],
        "shifts": [shift("a", &at(0, 7, 0), &at(0, 15, 0), Some("ann"))],
    })));
    // 3 h preferred x weight 3 x priority 2; 1 h unwanted x priority 2.
    assert_eq!(score(&plan, "Works in preferred hours"), soft(180 * 3 * 2));
    assert_eq!(score(&plan, "Works in unwanted hours"), soft(-60 * 2));
}

#[test]
fn bad_records_are_refused_with_codes() {
    let codes = issue_codes(dataset(json!({
        "employees": [employee("ann", json!({})), employee("ann", json!({}))],
        "shifts": [
            shift("a", "2027-03-01T07:00", "2027-03-01T06:00", Some("zed")),
            shift("a", "2027-03-01 07:00", "2027-03-01T08:00", None),
            shift("long", "2027-03-01T07:00", "2027-03-05T08:00", None),
        ],
    })));
    for code in [
        "duplicate_employee",
        "duplicate_shift",
        "shift_not_after_start",
        "unknown_employee",
        "invalid_date_time",
        "shift_too_long",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}
