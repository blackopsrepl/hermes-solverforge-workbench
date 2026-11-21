//! Busy periods: travel slower while they run, measured on the roads with
//! their slowdowns or brought by the dataset per period.

use serde_json::{json, Value};

use crate::common::*;

const AWAY: [f64; 2] = [45.5, 9.3];

fn soft(n: i64) -> String {
    format!("0hard/0medium/-{n}soft")
}

fn seconds(score: &str) -> i64 {
    score
        .trim_start_matches("0hard/0medium/-")
        .trim_end_matches("soft")
        .parse()
        .unwrap_or(0)
}

/// Ann leaves the depot at `hh:mm`, serves a ten-minute stop at [`AWAY`] and
/// comes back.
fn trip(hour: i64, minute: i64, extra: Value) -> Value {
    dataset(
        vec![driver(
            "ann",
            shift(
                "ann-mon",
                &["a"],
                json!({"minStartTime": at(0, hour, minute)}),
            ),
        )],
        vec![job(
            "J",
            vec![stop("a", 10, json!({"location": AWAY}))],
            json!({}),
        )],
        extra,
    )
}

fn travel_time(plan: Value) -> String {
    score(plan, "Travel time")
}

/// Ten minutes each way at usual speeds; twenty during the rush.
fn with_rush(traffic: Value) -> Value {
    json!({
        "travelMatrix": {
            "locations": [DEPOT, AWAY],
            "seconds": [[0, 600], [600, 0]],
            "meters": [[0, 5000], [5000, 0]],
            "periods": [{
                "id": "rush",
                "seconds": [[0, 1200], [1200, 0]],
                "meters": [[0, 6000], [6000, 0]],
            }],
        },
        "traffic": traffic,
    })
}

fn rush() -> Value {
    json!([{"id": "rush", "from": "08:00", "to": "09:00"}])
}

#[test]
fn a_dataset_matrix_per_busy_period_slows_the_legs_driven_in_it() {
    assert_eq!(travel_time(trip(10, 0, with_rush(rush()))), soft(1200));
    assert_eq!(travel_time(trip(8, 0, with_rush(rush()))), soft(2400));
    // Out at 07:55: half the way by 08:00, then ten rush minutes; back at
    // 08:20 for twenty.
    assert_eq!(
        travel_time(trip(7, 55, with_rush(rush()))),
        soft(900 + 1200)
    );
}

#[test]
fn a_busy_period_slows_the_roads_the_engine_measures() {
    // Long enough for the whole round trip (some 13 minutes each way).
    let traffic = json!({"traffic": [{
        "id": "rush", "from": "08:00", "to": "10:00",
        "slowdowns": [{"factor": 2.0}],
    }]});
    let quiet = seconds(&travel_time(trip(11, 0, traffic.clone())));
    let busy = seconds(&travel_time(trip(8, 0, traffic)));
    assert!(quiet > 0);
    assert!(
        (busy - 2 * quiet).abs() <= 2,
        "rush {busy} s, usual {quiet} s"
    );
}

#[test]
fn busy_periods_that_cannot_be_laid_out_or_travelled_are_refused() {
    let overlap = json!([{"id": "rush", "from": "08:00", "to": "09:00"},
                         {"id": "late", "from": "08:30", "to": "10:00"}]);
    assert!(issue_codes(trip(8, 0, with_rush(overlap))).contains(&"bad_traffic".to_string()));
    let unknown = json!([{"id": "evening", "from": "17:00", "to": "19:00"}]);
    assert!(
        issue_codes(trip(8, 0, with_rush(unknown))).contains(&"traffic_without_travel".to_string())
    );
}

#[test]
fn incremental_scores_match_full_ones_through_busy_periods() {
    let jobs: Vec<Value> = (0..4)
        .map(|i| {
            let f = i as f64;
            job(
                &format!("J{i}"),
                vec![
                    stop(
                        &format!("J{i}a"),
                        10,
                        json!({"location": [45.47 + 0.004 * f, 9.22 + 0.003 * f]}),
                    ),
                    stop(
                        &format!("J{i}b"),
                        10,
                        json!({"location": [45.49 - 0.003 * f, 9.25 - 0.002 * f]}),
                    ),
                ],
                json!({}),
            )
        })
        .collect();
    let wire = dataset(
        vec![
            driver(
                "ann",
                shift(
                    "ann-mon",
                    &["J0a", "J0b", "J1a", "J1b"],
                    json!({"minStartTime": at(0, 7, 40)}),
                ),
            ),
            driver("bob", shift("bob-mon", &["J2a", "J2b"], json!({}))),
        ],
        jobs,
        json!({"traffic": [
            {"id": "rush", "from": "08:00", "to": "09:00", "slowdowns": [{"factor": 2.5}]},
            {"id": "lunch", "from": "12:00", "to": "13:30", "slowdowns": [{"factor": 1.3}]},
        ]}),
    );
    assert_incremental_matches_full(prepared(wire), 300, 11);
}
