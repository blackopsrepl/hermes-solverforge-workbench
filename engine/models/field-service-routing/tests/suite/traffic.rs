//! Busy periods: travel slower while they run, measured on the roads with
//! their slowdowns or brought by the dataset per period.

use serde_json::{json, Value};

use crate::common::*;

const AWAY: [f64; 2] = [45.0793, 7.6869];

fn travel_time(plan: Value) -> String {
    score(plan, "Minimize travel time")
}

fn soft(n: i64) -> String {
    format!("0hard/0medium/-{n}soft")
}

/// Ann leaves the depot at `hh:mm`, serves a visit at [`AWAY`] for ten
/// minutes and comes back.
fn trip(hour: i64, minute: i64, extra: Value) -> Value {
    dataset(
        vec![vehicle(
            "ann",
            shift(
                "ann-mon",
                &["a"],
                json!({"minStartTime": at(0, hour, minute)}),
            ),
        )],
        vec![visit("a", 10, json!({"location": AWAY}))],
        extra,
    )
}

/// Ten minutes each way at usual speeds; twenty during the rush.
fn matrix_with_rush() -> Value {
    json!({
        "locations": [DEPOT, AWAY],
        "seconds": [[0, 600], [600, 0]],
        "meters": [[0, 5000], [5000, 0]],
        "periods": [{
            "id": "rush",
            "seconds": [[0, 1200], [1200, 0]],
            "meters": [[0, 6000], [6000, 0]],
        }],
    })
}

fn rush(from: &str, to: &str) -> Value {
    json!({"id": "rush", "from": from, "to": to})
}

#[test]
fn a_dataset_matrix_per_busy_period_slows_the_legs_driven_in_it() {
    let extra = |traffic: Value| json!({"travelMatrix": matrix_with_rush(), "traffic": traffic});
    // Out of the rush: ten minutes each way.
    assert_eq!(
        travel_time(trip(10, 0, extra(json!([rush("08:00", "09:00")])))),
        soft(1200)
    );
    // Both legs inside it: twenty each way.
    assert_eq!(
        travel_time(trip(8, 0, extra(json!([rush("08:00", "09:00")])))),
        soft(2400)
    );
    // Out at 07:55, half of the way by 08:00 then ten rush minutes; back at
    // 08:20 (ten minutes of service) for twenty.
    assert_eq!(
        travel_time(trip(7, 55, extra(json!([rush("08:00", "09:00")])))),
        soft(900 + 1200)
    );
    // Not on Mondays.
    let tuesdays = json!([{"id": "rush", "from": "08:00", "to": "09:00", "days": ["tuesday"]}]);
    assert_eq!(travel_time(trip(8, 0, extra(tuesdays))), soft(1200));
}

#[test]
fn a_busy_period_slows_the_roads_the_engine_measures() {
    // The test roads carry no class: a slowdown naming no roads applies to all.
    let traffic = json!({"traffic": [{
        "id": "rush", "from": "08:00", "to": "09:00",
        "slowdowns": [{"factor": 2.0}],
    }]});
    let quiet = travel_time(trip(10, 0, traffic.clone()));
    let busy = travel_time(trip(8, 0, traffic.clone()));
    let seconds = |s: &str| -> i64 {
        s.trim_start_matches("0hard/0medium/-")
            .trim_end_matches("soft")
            .parse()
            .unwrap()
    };
    let (quiet, busy) = (seconds(&quiet), seconds(&busy));
    assert!(quiet > 0);
    assert!(
        (busy - 2 * quiet).abs() <= 2,
        "rush {busy} s, usual {quiet} s"
    );
    // A slowdown limited to motorways leaves these roads alone.
    let motorways = json!({"traffic": [{
        "id": "rush", "from": "08:00", "to": "09:00",
        "slowdowns": [{"factor": 2.0, "roads": ["motorway"]}],
    }]});
    assert_eq!(seconds(&travel_time(trip(8, 0, motorways))), quiet);
}

#[test]
fn the_engine_keeps_the_busy_periods_it_measured_with_the_plan() {
    let wire = trip(
        8,
        0,
        json!({"traffic": [{"id": "rush", "from": "08:00", "to": "09:00",
                            "slowdowns": [{"factor": 1.5, "roads": ["other"]}]}]}),
    );
    give_roads(&wire);
    let (code, answer) = engine(json!({"command": "roads", "plan": wire}));
    assert_eq!(code, 0, "{answer}");
    let matrix = &answer["plan"]["travelMatrix"];
    assert_eq!(matrix["source"], "roads");
    let period = &matrix["periods"][0];
    assert_eq!(period["id"], "rush");
    assert_eq!(period["slowdowns"][0]["factor"], 1.5);
    let usual = matrix["seconds"][0][1].as_f64().unwrap();
    let slowed = period["seconds"][0][1].as_f64().unwrap();
    assert!((slowed - usual * 1.5).abs() <= 1.0, "{slowed} vs {usual}");
}

#[test]
fn busy_periods_that_cannot_be_laid_out_or_travelled_are_refused() {
    let with = |traffic: Value| json!({"travelMatrix": matrix_with_rush(), "traffic": traffic});
    let overlap =
        with(json!([rush("08:00", "09:00"), {"id": "late", "from": "08:30", "to": "10:00"}]));
    assert!(issue_codes(trip(8, 0, overlap)).contains(&"bad_traffic".to_string()));
    let malformed = with(json!([rush("8:00", "09:00")]));
    assert!(issue_codes(trip(8, 0, malformed)).contains(&"bad_traffic".to_string()));
    let unknown = with(json!([{"id": "evening", "from": "17:00", "to": "19:00"}]));
    assert!(issue_codes(trip(8, 0, unknown)).contains(&"traffic_without_travel".to_string()));
    let mut closed = matrix_with_rush();
    closed["periods"][0]["seconds"][0][1] = Value::Null;
    let closes = json!({"travelMatrix": closed, "traffic": [rush("08:00", "09:00")]});
    assert!(issue_codes(trip(8, 0, closes)).contains(&"bad_travel_matrix".to_string()));
}

#[test]
fn incremental_scores_match_full_ones_through_busy_periods() {
    let visits: Vec<Value> = (0..6)
        .map(|i| {
            visit(
                &format!("v{i}"),
                15,
                json!({"location": [45.06 + 0.004 * i as f64, 7.68 + 0.003 * i as f64]}),
            )
        })
        .collect();
    let wire = dataset(
        vec![
            vehicle(
                "ann",
                shift(
                    "ann-mon",
                    &["v0", "v1", "v2"],
                    json!({"minStartTime": at(0, 7, 40)}),
                ),
            ),
            vehicle("bob", shift("bob-mon", &["v3", "v4"], json!({}))),
        ],
        visits,
        json!({"traffic": [
            {"id": "rush", "from": "08:00", "to": "09:00", "slowdowns": [{"factor": 2.5}]},
            {"id": "lunch", "from": "12:00", "to": "13:30", "slowdowns": [{"factor": 1.3}]},
        ]}),
    );
    assert_incremental_matches_full(prepared(wire), 300, 7);
}
