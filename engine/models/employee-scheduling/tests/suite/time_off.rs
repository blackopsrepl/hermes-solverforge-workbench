//! Time-off rules, each against a hand-built plan with a known penalty.

use crate::common::*;
use serde_json::{json, Value};
use solverforge_workbench_employee_scheduling::Plan;

fn under(rules: Value, employees: Vec<Value>, shifts: Vec<Value>, end: &str) -> Plan {
    plan(dataset(json!({
        "schedule": {"start": "2027-03-01T00:00", "end": end},
        "contracts": [{"id": "c", "rules": rules}],
        "employees": employees,
        "shifts": shifts,
    })))
}

fn on(id: &str) -> Value {
    employee(id, json!({"contracts": ["c"]}))
}

fn day_shift(id: &str, day: i64, who: &str) -> Value {
    shift(id, &at(day, 7, 0), &at(day, 15, 0), Some(who))
}

const WEEK: &str = "2027-03-08T00:00";

#[test]
fn days_off_per_period() {
    let mut shifts: Vec<Value> = (0..5)
        .map(|d| day_shift(&format!("a{d}"), d, "ann"))
        .collect();
    shifts.push(day_shift("c5", 5, "carl"));
    let plan = under(
        json!([
            {"kind": "daysOff", "period": "week", "min": 3, "max": 5},
            {"kind": "daysOff", "period": "week", "level": "preferred", "weekdays": ["saturday", "sunday"], "min": 2},
        ]),
        vec![on("ann"), on("bob"), on("carl")],
        shifts,
        WEEK,
    );
    // ann: 2 days off; bob: 7; carl: 6, one of them on the weekend.
    assert_eq!(
        score(&plan, "Days off per period outside required range"),
        hard(-4)
    );
    assert_eq!(
        score(&plan, "Days off per period outside preferred range"),
        soft(-1)
    );
}

#[test]
fn consecutive_days_off_per_period() {
    let plan = under(
        json!([
            {"kind": "consecutiveDaysOff", "period": "week", "min": 3},
            {"kind": "consecutiveDaysOff", "period": "week", "level": "preferred", "min": 4, "weight": 3},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            day_shift("a", 0, "ann"),
            day_shift("b", 3, "ann"),
            day_shift("c", 6, "ann"),
        ],
        WEEK,
    );
    assert_eq!(
        score(
            &plan,
            "Consecutive days off per period below required minimum"
        ),
        hard(-1)
    );
    assert_eq!(
        score(
            &plan,
            "Consecutive days off per period below preferred minimum"
        ),
        soft(-6)
    );
}

#[test]
fn consecutive_days_off_in_rolling_window() {
    let plan = under(
        json!([
            {"kind": "consecutiveDaysOff", "rollingDays": 3, "min": 2},
            {"kind": "consecutiveDaysOff", "rollingDays": 7, "level": "preferred", "min": 3},
        ]),
        vec![on("ann"), on("bob")],
        vec![
            day_shift("a", 0, "ann"),
            day_shift("b", 3, "ann"),
            day_shift("c", 6, "ann"),
        ],
        WEEK,
    );
    // Only the Wed-Fri window lacks two days off in a row.
    assert_eq!(
        score(
            &plan,
            "Consecutive days off in rolling window below required minimum"
        ),
        hard(-1)
    );
    assert_eq!(
        score(
            &plan,
            "Consecutive days off in rolling window below preferred minimum"
        ),
        soft(-1)
    );
}

#[test]
fn continuous_rest_in_rolling_window() {
    let plan = under(
        json!([
            {"kind": "consecutiveMinutesOff", "rollingDays": 2, "min": 1800},
            {"kind": "consecutiveMinutesOff", "rollingDays": 1, "level": "preferred", "min": 1200},
        ]),
        vec![on("ann")],
        vec![day_shift("a", 0, "ann"), day_shift("b", 1, "ann")],
        WEEK,
    );
    // Mon-Tue: longest rest is 15:00 to 07:00, 960 minutes.
    assert_eq!(
        score(
            &plan,
            "Continuous rest in rolling window below required minimum"
        ),
        hard(-840)
    );
    // Mon and Tue alone: longest rest 15:00 to midnight, 540 minutes.
    assert_eq!(
        score(
            &plan,
            "Continuous rest in rolling window below preferred minimum"
        ),
        soft(-1320)
    );
}

#[test]
fn consecutive_weekends_off_per_period() {
    let plan = under(
        json!([
            {"kind": "consecutiveWeekendsOff", "period": "schedule", "min": 2},
            {"kind": "consecutiveWeekendsOff", "period": "schedule", "level": "preferred", "min": 3, "weight": 2},
        ]),
        vec![on("ann"), on("bob")],
        vec![day_shift("a", 12, "ann"), day_shift("b", 26, "ann")],
        "2027-03-29T00:00",
    );
    assert_eq!(
        score(
            &plan,
            "Consecutive weekends off per period below required minimum"
        ),
        hard(-1)
    );
    assert_eq!(
        score(
            &plan,
            "Consecutive weekends off per period below preferred minimum"
        ),
        soft(-4)
    );
}

#[test]
fn shifts_next_to_a_requested_day_off() {
    let plan = under(
        json!([
            {"kind": "nearDayOff", "stance": "barred", "before": {"withTags": ["night"]}},
            {"kind": "nearDayOff", "stance": "unwanted", "after": {"withTags": ["early"]}, "weight": 2},
        ]),
        vec![employee(
            "ann",
            json!({"contracts": ["c"], "daysOff": ["2027-03-03"]}),
        )],
        vec![
            shift_with(
                "tue",
                &at(1, 23, 0),
                &at(2, 7, 0),
                Some("ann"),
                json!({"tags": ["night"]}),
            ),
            shift_with(
                "thu",
                &at(3, 7, 0),
                &at(3, 15, 0),
                Some("ann"),
                json!({"tags": ["early"]}),
            ),
            shift_with(
                "thu2",
                &at(3, 15, 0),
                &at(3, 23, 0),
                Some("ann"),
                json!({"tags": ["late"]}),
            ),
        ],
        WEEK,
    );
    assert_eq!(
        score(&plan, "Barred shift next to a requested day off"),
        hard(-1)
    );
    assert_eq!(
        score(&plan, "Unwanted shift next to a requested day off"),
        soft(-2)
    );
}

#[test]
fn rosters_score_incrementally_like_in_full() {
    let mut shifts = Vec::new();
    for d in 0..14 {
        shifts.push(shift_with(
            &format!("e{d}"),
            &at(d, 7, 0),
            &at(d, 15, 0),
            None,
            json!({"tags": ["day"]}),
        ));
        shifts.push(shift_with(
            &format!("n{d}"),
            &at(d, 23, 0),
            &at(d + 1, 7, 0),
            None,
            json!({"tags": ["night"]}),
        ));
    }
    let plan = under(
        json!([
            {"kind": "minutesWorked", "period": "week", "max": 2400},
            {"kind": "minutesWorked", "rollingDays": 3, "level": "preferred", "min": 480},
            {"kind": "minutesLogged", "period": "week", "level": "preferred", "max": 1500},
            {"kind": "daysWorked", "rollingDays": 7, "max": 5},
            {"kind": "shiftsWorked", "period": "schedule", "level": "preferred", "min": 6},
            {"kind": "weekendsWorked", "rollingDays": 14, "max": 1},
            {"kind": "weekendMinutes", "period": "week", "level": "preferred", "max": 480},
            {"kind": "consecutiveWeekendsWorked", "period": "schedule", "max": 1},
            {"kind": "consecutiveDaysWorked", "max": 3, "categories": ["day", "night"], "startDays": ["monday"], "minGapMinutes": 2000},
            {"kind": "consecutiveDaysWorked", "level": "preferred", "min": 2, "compact": true, "categories": ["day", "night"], "startDays": ["tuesday"]},
            {"kind": "consecutiveShifts", "max": 3, "restAfterMinutes": 2000, "restAfterRunsOf": 2},
            {"kind": "daysOff", "period": "week", "level": "preferred", "min": 2},
            {"kind": "consecutiveDaysOff", "rollingDays": 7, "min": 2},
            {"kind": "consecutiveMinutesOff", "rollingDays": 2, "level": "preferred", "min": 1500},
            {"kind": "consecutiveWeekendsOff", "period": "schedule", "min": 1},
            {"kind": "nearDayOff", "stance": "barred", "before": {"withTags": ["night"]}},
        ]),
        vec![
            employee(
                "ann",
                json!({"contracts": ["c"], "daysOff": ["2027-03-05"]}),
            ),
            on("bob"),
            on("cy"),
        ],
        shifts,
        "2027-03-15T00:00",
    );
    assert_incremental_matches_full(plan, 600, 3);
}
