//! Working together, travel and locations, each against a hand-built plan
//! with a known penalty.

use crate::common::*;
use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::wire::Place;

fn tagged(id: &str, day: i64, from: (i64, i64), to: (i64, i64), who: &str, tags: &[&str]) -> Value {
    let end_day = if to.0 < from.0 { day + 1 } else { day };
    shift_with(
        id,
        &at(day, from.0, from.1),
        &at(end_day, to.0, to.1),
        Some(who),
        json!({"tags": tags}),
    )
}

#[test]
fn pairings_between_employees() {
    let plan = plan(dataset(json!({
        "employees": [
            employee("ann", json!({"priority": 2, "pairings": [
                {"stance": "required", "with": "bob"},
                {"stance": "preferred", "withTags": ["mentor"]},
                {"stance": "unwanted", "with": "dan"},
                {"stance": "barred", "with": "eve", "onTags": ["icu"]},
            ]})),
            employee("bob", json!({})),
            employee("cy", json!({"tags": ["mentor"]})),
            employee("dan", json!({})),
            employee("eve", json!({})),
        ],
        "shifts": [
            tagged("a1", 0, (7, 0), (15, 0), "ann", &[]),
            tagged("b1", 0, (8, 0), (12, 0), "bob", &[]),
            tagged("c1", 0, (14, 0), (18, 0), "cy", &[]),
            tagged("d1", 0, (13, 0), (14, 0), "dan", &[]),
            tagged("a2", 1, (7, 0), (15, 0), "ann", &["icu"]),
            tagged("e2", 1, (10, 0), (11, 0), "eve", &[]),
            tagged("a3", 2, (22, 0), (6, 0), "ann", &[]),
            tagged("b3", 3, (5, 0), (9, 0), "bob", &[]),
            tagged("a4", 4, (7, 0), (15, 0), "ann", &["ward"]),
            tagged("e4", 4, (8, 0), (9, 0), "eve", &[]),
        ],
    })));
    // Tuesday and Friday without bob; the overnight shift meets bob after midnight.
    assert_eq!(score(&plan, "Required partner absent"), hard(-2));
    assert_eq!(score(&plan, "Preferred partner present"), soft(2));
    assert_eq!(score(&plan, "Unwanted partner present"), soft(-2));
    // eve only counts on icu shifts.
    assert_eq!(score(&plan, "Barred partner present"), hard(-1));
}

#[test]
fn teams_work_together() {
    let plan = plan(dataset(json!({
        "tags": [{"id": "room-1", "type": "room"}, {"id": "room-2", "type": "room"}],
        "teams": [{"id": "t", "leads": ["lea"], "members": ["mo", "mi"], "tagType": "room", "maxShiftsPerDay": 1}],
        "employees": [employee("lea", json!({})), employee("mo", json!({})), employee("mi", json!({}))],
        "shifts": [
            tagged("l0", 0, (7, 0), (15, 0), "lea", &["room-1"]),
            tagged("o0", 0, (8, 0), (16, 0), "mo", &["room-1"]),
            tagged("i0", 0, (9, 0), (12, 0), "mi", &["room-2"]),
            tagged("o1", 1, (7, 0), (15, 0), "mo", &["room-1"]),
        ],
    })));
    assert_eq!(score(&plan, "Team members split across tags"), hard(-2));
    assert_eq!(score(&plan, "Team member without a lead"), hard(-1));
    assert_eq!(score(&plan, "Team lead works with the team"), soft(1));
    // Monday room-1 is worked 07-15 and 08-16.
    assert_eq!(score(&plan, "Team shifts per day over limit"), hard(-1));
}

#[test]
fn travel_distance_and_location_changes() {
    let brussels = Place {
        lat: 50.8503,
        lon: 4.3517,
    };
    let antwerp = Place {
        lat: 51.2194,
        lon: 4.4025,
    };
    let metres = brussels.metres_to(&antwerp);
    let plan = plan(dataset(json!({
        "contracts": [{"id": "c", "rules": [
            {"kind": "travel", "maxMeters": 20000, "minChangeMinutes": 120},
            {"kind": "locationsWorked", "period": "week", "max": 1},
            {"kind": "locationsWorked", "period": "week", "level": "preferred", "max": 1, "weight": 3},
        ]}],
        "employees": [employee("ann", json!({"contracts": ["c"], "home": brussels}))],
        "shifts": [
            shift_with("s1", &at(0, 7, 0), &at(0, 15, 0), Some("ann"), json!({"location": antwerp})),
            shift_with("s2", &at(0, 15, 30), &at(0, 18, 0), Some("ann"), json!({"location": brussels})),
        ],
    })));
    let over = ((metres - 20000.0) / 1000.0).ceil() as i64;
    assert_eq!(score(&plan, "Travel distance over limit"), hard(-over));
    assert_eq!(
        score(&plan, "Travel distance"),
        soft(-((metres / 1000.0).round() as i64))
    );
    assert_eq!(
        score(&plan, "Too little time to change location"),
        hard(-90)
    );
    assert_eq!(
        score(&plan, "Locations per period outside required range"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Locations per period outside preferred range"),
        soft(-3)
    );
}

#[test]
fn pairing_targets_are_validated() {
    let codes = issue_codes(dataset(json!({
        "employees": [employee("ann", json!({"pairings": [
            {"stance": "required"},
            {"stance": "required", "with": "ann"},
            {"stance": "required", "with": "zed"},
        ]}))],
        "teams": [{"id": "t", "members": ["ann"], "maxShiftsPerDay": 1}],
    })));
    for code in [
        "pairing_target_invalid",
        "pairing_with_self",
        "unknown_employee",
        "team_needs_tag_type",
    ] {
        assert!(codes.contains(&code), "missing {code} in {codes:?}");
    }
}

#[test]
fn working_together_scores_incrementally_like_in_full() {
    let places = [
        json!({"lat": 50.85, "lon": 4.35}),
        json!({"lat": 51.22, "lon": 4.40}),
        json!({"lat": 51.05, "lon": 3.72}),
    ];
    let mut shifts = Vec::new();
    for d in 0..7 {
        for (n, (from, to)) in [
            ((7, 0), (15, 0)),
            ((8, 0), (12, 0)),
            ((15, 0), (23, 0)),
            ((22, 0), (6, 0)),
        ]
        .iter()
        .enumerate()
        {
            let mut record = tagged(
                &format!("s{d}-{n}"),
                d,
                *from,
                *to,
                "x",
                &[["room-1", "room-2"][n % 2], "icu"],
            );
            record["employee"] = Value::Null;
            record["location"] = places[(d as usize + n) % 3].clone();
            shifts.push(record);
        }
    }
    let plan = plan(dataset(json!({
        "tags": [{"id": "room-1", "type": "room"}, {"id": "room-2", "type": "room"}],
        "contracts": [{"id": "c", "rules": [
            {"kind": "travel", "maxMeters": 30000, "minChangeMinutes": 60},
            {"kind": "locationsWorked", "period": "week", "level": "preferred", "max": 2},
        ]}],
        "teams": [{"id": "t", "leads": ["ann"], "members": ["bob", "cy"], "tagType": "room", "maxShiftsPerDay": 1}],
        "employees": [
            employee("ann", json!({"contracts": ["c"], "home": places[0], "pairings": [{"stance": "required", "with": "bob", "onTags": ["icu"]}]})),
            employee("bob", json!({"contracts": ["c"], "home": places[1], "pairings": [{"stance": "barred", "with": "dee"}]})),
            employee("cy", json!({"pairings": [{"stance": "preferred", "withTags": ["senior"]}, {"stance": "unwanted", "with": "ann"}]})),
            employee("dee", json!({"tags": ["senior"], "home": places[2]})),
        ],
        "shifts": shifts,
    })));
    assert_incremental_matches_full(plan, 600, 13);
}
